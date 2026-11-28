use std::f32::consts::TAU;

use dh_voice_client::consts::dsp::{
    DSP_FRAME_SAMPLES_10MS, LIMITER_CEILING, LIMITER_KNEE_THRESHOLD, LOCAL_ENERGY_ATTACK_ALPHA,
    LOCAL_ENERGY_RELEASE_ALPHA, VAD_HOLDOVER_FRAMES, VOICE_SAMPLE_RATE_HZ,
};
use dh_voice_client::consts::ue4::SPEAKER_ACTIVE_WINDOW_MS;
use dh_voice_client::consts::wasapi::{
    HRESULT_AUDCLNT_E_DEVICE_INVALIDATED, HRESULT_AUDCLNT_E_RESOURCES_INVALIDATED,
    HRESULT_AUDCLNT_E_SERVICE_NOT_RUNNING,
};
use dh_voice_client::{
    decay_meter_energy, decode_interleaved_to_mono_f32, is_recoverable_device_error,
    normalized_f32_to_i16, pcm_i16_rms_energy, rms_to_meter_energy, smooth_meter_energy,
    soft_knee_limit, AgcLimiterState, BiquadHpfState, DspFrameResult, DspPipeline, LinearResampler,
    RenderReferenceRing, VadGateState, WasapiSampleFormat, WasapiStreamFormat,
};

#[test]
fn normalized_f32_to_i16_is_exact_and_safe() {
    assert_eq!(normalized_f32_to_i16(0.0), 0);
    assert_eq!(normalized_f32_to_i16(1.0), 32_767);
    assert_eq!(normalized_f32_to_i16(-1.0), -32_767);
    assert_eq!(normalized_f32_to_i16(2.5), 32_767);
    assert_eq!(normalized_f32_to_i16(-2.5), -32_767);
    assert_eq!(normalized_f32_to_i16(f32::NAN), 0);
    assert_eq!(normalized_f32_to_i16(f32::INFINITY), 0);
    assert_eq!(normalized_f32_to_i16(f32::NEG_INFINITY), 0);
    assert!((i32::from(normalized_f32_to_i16(0.5)) - 16_384).abs() <= 1);
}

#[test]
fn soft_knee_limiter_preserves_linear_region_and_caps_peaks() {
    assert!((soft_knee_limit(0.5) - 0.5).abs() < 1.0e-6);
    assert!((soft_knee_limit(-0.5) + 0.5).abs() < 1.0e-6);
    assert!((soft_knee_limit(LIMITER_KNEE_THRESHOLD) - LIMITER_KNEE_THRESHOLD).abs() < 1.0e-6);

    let loud_pos: f32 = soft_knee_limit(5.0);
    let loud_neg: f32 = soft_knee_limit(-5.0);
    assert!(loud_pos > LIMITER_KNEE_THRESHOLD && loud_pos < LIMITER_CEILING);
    assert!((loud_pos + loud_neg).abs() < 1.0e-6);
    assert_eq!(soft_knee_limit(f32::NAN), 0.0);
}

#[test]
fn butterworth_80hz_hpf_attenuates_dc_and_low_rumble_but_passes_1khz() {
    let mut hpf_dc: BiquadHpfState = BiquadHpfState::default();
    let mut last_dc: f32 = 1.0;
    for _idx in 0..4800 {
        last_dc = hpf_dc.process_sample(0.8);
    }
    assert!(
        last_dc.abs() < 0.005,
        "DC offset should settle near zero, got {last_dc}"
    );

    // 20 Hz rumble should be attenuated > 20 dB (~ < 0.1 amplitude)
    let mut hpf_low: BiquadHpfState = BiquadHpfState::default();
    let mut rumble_peak: f32 = 0.0;
    for idx in 0..9600 {
        let t: f32 = f32::from(u16::try_from(idx).unwrap()) / 48_000.0;
        let y: f32 = hpf_low.process_sample((TAU * 20.0 * t).sin());
        if idx >= 4800 {
            rumble_peak = rumble_peak.max(y.abs());
        }
    }
    assert!(
        rumble_peak < 0.12,
        "20 Hz rumble not attenuated: {rumble_peak}"
    );

    // 1 kHz voice band should pass with nearly unity gain (> 0.98)
    let mut hpf_mid: BiquadHpfState = BiquadHpfState::default();
    let mut passband_peak: f32 = 0.0;
    for idx in 0..4800 {
        let t: f32 = f32::from(u16::try_from(idx).unwrap()) / 48_000.0;
        let y: f32 = hpf_mid.process_sample((TAU * 1000.0 * t).sin());
        if idx >= 2400 {
            passband_peak = passband_peak.max(y.abs());
        }
    }
    assert!(
        passband_peak > 0.98,
        "1 kHz voice band attenuated: {passband_peak}"
    );
}

#[test]
fn vad_gate_holds_open_for_240ms_then_smoothly_closes() {
    let mut vad: VadGateState = VadGateState::default();
    let (_p0, g0, open0): (f32, f32, bool) = vad.update(0.85, 0.08);
    assert!(open0);
    assert!(g0 > 0.5);

    // 23 silent frames must remain open inside the 240 ms holdover window
    for frame in 0..(VAD_HOLDOVER_FRAMES - 1) {
        let (_p, _g, open): (f32, f32, bool) = vad.update(0.02, 0.0001);
        assert!(open, "VAD closed prematurely during holdover frame {frame}");
    }

    // After holdover expires and release tail decays, gate must close cleanly to 0.0
    let mut closed: bool = false;
    for _frame in 0..60 {
        let (_p, g, open): (f32, f32, bool) = vad.update(0.02, 0.0001);
        if !open && g == 0.0 {
            closed = true;
            break;
        }
    }
    assert!(closed, "VAD gate failed to close after holdover + release");
}

#[test]
fn agc_boosts_quiet_speech_and_attenuates_loud_speech() {
    let mut agc: AgcLimiterState = AgcLimiterState::default();
    for _ in 0..40 {
        let _g: (f32, f32) = agc.update(0.02, true);
    }
    assert!(agc.current_gain > 1.5, "AGC did not boost quiet speech");

    let boosted: f32 = agc.current_gain;
    let (_p, unchanged): (f32, f32) = agc.update(0.0005, false);
    assert!((unchanged - boosted).abs() < 1.0e-6);

    for _ in 0..20 {
        let _g: (f32, f32) = agc.update(0.45, true);
    }
    assert!(
        agc.current_gain < 0.8,
        "AGC did not attenuate loud shouting"
    );
}

#[test]
fn full_dsp_pipeline_processes_echo_speech_and_silence_cleanly() {
    let mut dsp: DspPipeline = DspPipeline::new();
    let mut render: [f32; DSP_FRAME_SAMPLES_10MS] = [0.0; DSP_FRAME_SAMPLES_10MS];
    let mut capture: [f32; DSP_FRAME_SAMPLES_10MS] = [0.0; DSP_FRAME_SAMPLES_10MS];

    // Warm up AEC3 while PTT is off
    for idx in 0..DSP_FRAME_SAMPLES_10MS {
        let t: f32 = f32::from(u16::try_from(idx).unwrap()) / 48_000.0;
        render[idx] = 0.25 * (TAU * 440.0 * t).sin();
        capture[idx] = 0.10 * render[idx];
    }
    for _ in 0..5 {
        dsp.warmup_aec_10ms(&capture, Some(&render));
    }

    // Active speech + echo frames through full chain
    for frame in 0..10u16 {
        for idx in 0..DSP_FRAME_SAMPLES_10MS {
            let sample_num: f32 = f32::from(frame) * 480.0 + f32::from(u16::try_from(idx).unwrap());
            let t: f32 = sample_num / 48_000.0;
            render[idx] = 0.2 * (TAU * 300.0 * t).sin();
            capture[idx] = 0.25 * (TAU * 600.0 * t).sin() + 0.05 * render[idx];
        }
        let res: DspFrameResult = dsp.process_capture_10ms(&capture, Some(&render));
        assert!(res.vad_probability.is_finite());
        assert!((0.0..=1.0).contains(&res.vad_probability));
    }

    // Pure silence frames (700 ms > 240 ms holdover + ~270 ms smooth release tail) must settle cleanly
    let silent: [f32; DSP_FRAME_SAMPLES_10MS] = [0.0; DSP_FRAME_SAMPLES_10MS];
    for _ in 0..70 {
        let res: DspFrameResult = dsp.process_capture_10ms(&silent, Some(&silent));
        assert!(res.vad_probability.is_finite());
    }
    let tail: DspFrameResult = dsp.process_capture_10ms(&silent, Some(&silent));
    assert!(!tail.speech_active);
    assert_eq!(tail.pcm_i16, [0i16; DSP_FRAME_SAMPLES_10MS]);
}

#[test]
fn wasapi_resampler_ring_and_pcm_decoders_work_accurately() {
    // 48 kHz passthrough
    let mut pass: LinearResampler =
        LinearResampler::new(VOICE_SAMPLE_RATE_HZ, VOICE_SAMPLE_RATE_HZ);
    let mut out_pass: Vec<f32> = Vec::new();
    pass.process(&[0.25, -0.5, 0.75], &mut out_pass);
    assert_eq!(out_pass, vec![0.25, -0.5, 0.75]);

    // 44.1 kHz -> 48 kHz ratio
    let mut up: LinearResampler = LinearResampler::new(44_100, 48_000);
    let input_44k: Vec<f32> = vec![0.5; 4410];
    let mut out_48k: Vec<f32> = Vec::new();
    up.process(&input_44k, &mut out_48k);
    assert!(
        (i32::try_from(out_48k.len()).unwrap() - 4800).abs() <= 2,
        "unexpected resampled length: {}",
        out_48k.len()
    );
    assert!(out_48k.iter().all(|s: &f32| (*s - 0.5).abs() < 1.0e-5));

    // Stereo Int16 downmix
    let mut stereo_i16_bytes: Vec<u8> = Vec::new();
    stereo_i16_bytes.extend_from_slice(&16_384i16.to_le_bytes());
    stereo_i16_bytes.extend_from_slice(&(-16_384i16).to_le_bytes());
    stereo_i16_bytes.extend_from_slice(&16_384i16.to_le_bytes());
    stereo_i16_bytes.extend_from_slice(&16_384i16.to_le_bytes());
    let mut mono: Vec<f32> = Vec::new();
    decode_interleaved_to_mono_f32(
        &stereo_i16_bytes,
        2,
        WasapiStreamFormat {
            sample_rate: 48_000,
            channels: 2,
            block_align: 4,
            sample_format: WasapiSampleFormat::Int16,
        },
        &mut mono,
    );
    assert_eq!(mono.len(), 2);
    assert!(mono[0].abs() < 1.0e-5);
    assert!((mono[1] - 0.5).abs() < 1.0e-4);

    // Bounded RenderReferenceRing drops oldest when full
    let ring: RenderReferenceRing = RenderReferenceRing::new(2);
    ring.push_frame([1.0; DSP_FRAME_SAMPLES_10MS]);
    ring.push_frame([2.0; DSP_FRAME_SAMPLES_10MS]);
    ring.push_frame([3.0; DSP_FRAME_SAMPLES_10MS]);
    assert_eq!(ring.pop_frame().unwrap()[0], 2.0);
    assert_eq!(ring.pop_frame().unwrap()[0], 3.0);
    assert!(ring.pop_frame().is_none());

    // Recoverable WASAPI device error classification
    assert!(is_recoverable_device_error(
        HRESULT_AUDCLNT_E_DEVICE_INVALIDATED
    ));
    assert!(is_recoverable_device_error(
        HRESULT_AUDCLNT_E_SERVICE_NOT_RUNNING
    ));
    assert!(is_recoverable_device_error(
        HRESULT_AUDCLNT_E_RESOURCES_INVALIDATED
    ));
    assert!(!is_recoverable_device_error(0));
}

#[test]
fn perceptual_mic_energy_meter_maps_smooths_and_decays_cleanly() {
    // Silence, empty, sub-floor noise, and non-finite inputs map strictly to 0.0
    assert_eq!(pcm_i16_rms_energy(&[]), 0.0);
    assert_eq!(pcm_i16_rms_energy(&[0i16; DSP_FRAME_SAMPLES_10MS]), 0.0);
    assert_eq!(rms_to_meter_energy(0.0), 0.0);
    assert_eq!(rms_to_meter_energy(0.001), 0.0); // -60 dBFS < -52 dBFS floor
    assert_eq!(rms_to_meter_energy(f32::NAN), 0.0);
    assert_eq!(rms_to_meter_energy(f32::INFINITY), 0.0);

    // Conversational speech PCM (~ -20 dBFS = amplitude ~3276) maps into mid-high (0.4, 0.9)
    let speech_pcm: [i16; DSP_FRAME_SAMPLES_10MS] = [3_276i16; DSP_FRAME_SAMPLES_10MS];
    let speech_energy: f32 = pcm_i16_rms_energy(&speech_pcm);
    assert!(
        speech_energy > 0.45 && speech_energy < 0.90,
        "unexpected speech meter energy: {speech_energy}"
    );

    // Clipping / loud speech (>= -8 dBFS) saturates cleanly at 1.0
    let loud_pcm: [i16; DSP_FRAME_SAMPLES_10MS] = [i16::MAX; DSP_FRAME_SAMPLES_10MS];
    assert!((pcm_i16_rms_energy(&loud_pcm) - 1.0).abs() < 1.0e-6);

    // Smoothing rises quickly on attack and releases smoothly toward 0.0
    let attacked: f32 = smooth_meter_energy(
        0.0,
        0.8,
        LOCAL_ENERGY_ATTACK_ALPHA,
        LOCAL_ENERGY_RELEASE_ALPHA,
    );
    assert!(attacked > 0.45 && attacked < 0.8);
    let mut releasing: f32 = attacked;
    for _ in 0..50 {
        releasing = smooth_meter_energy(
            releasing,
            0.0,
            LOCAL_ENERGY_ATTACK_ALPHA,
            LOCAL_ENERGY_RELEASE_ALPHA,
        );
    }
    assert_eq!(releasing, 0.0);

    // Time-based stale decay glides smoothly from 100% -> 50% -> 0% over SPEAKER_ACTIVE_WINDOW_MS
    let full: f32 = decay_meter_energy(0.8, 0, SPEAKER_ACTIVE_WINDOW_MS);
    let half: f32 = decay_meter_energy(0.8, SPEAKER_ACTIVE_WINDOW_MS / 2, SPEAKER_ACTIVE_WINDOW_MS);
    let expired: f32 = decay_meter_energy(0.8, SPEAKER_ACTIVE_WINDOW_MS, SPEAKER_ACTIVE_WINDOW_MS);
    assert!((full - 0.8).abs() < 1.0e-5);
    assert!((half - 0.4).abs() < 0.02);
    assert_eq!(expired, 0.0);
}
