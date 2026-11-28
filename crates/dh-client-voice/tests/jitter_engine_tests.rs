use std::ffi::{c_char, c_void};
use std::time::{Duration, Instant};

use dh_voice_client::consts::dsp::OPUS_FRAME_SAMPLES_20MS;
use dh_voice_client::consts::engine::{
    SOCKADDR_AF_INET, SOCKADDR_AF_INET6, SOCKADDR_INET6_LEN, SOCKADDR_INET_LEN,
    SOCKADDR_STORAGE_SIZE,
};
use dh_voice_client::consts::jitter::{
    JITTER_MAX_BUFFERED_FRAMES, JITTER_MAX_PLAYOUT_FRAMES_PER_TICK, JITTER_MAX_TARGET_FRAMES,
    JITTER_MIN_TARGET_FRAMES,
};
use dh_voice_client::{
    advance_playout_accumulator, apply_plc_fade_in_place, encode_uid_utf16_nul,
    parse_sockaddr_storage, plc_fade_gain, seq_diff, DecodedPlayoutFrame, OpusApiTable,
    OpusDecoder, OpusEncoder, PlayoutAction, SpeakerJitterBuffer, VoiceRxState,
};
use dh_voice_proto::{PeerMapEntry, FLAG_END_OF_TALKSPURT, FLAG_HAS_FEC};

unsafe extern "C" fn mock_get_size(_channels: i32) -> i32 {
    64
}

unsafe extern "C" fn mock_enc_init(_state: *mut c_void, _fs: i32, _ch: i32, _app: i32) -> i32 {
    0
}

unsafe extern "C" fn mock_enc_ctl_set(_state: *mut c_void, _req: i32, _val: i32) -> i32 {
    0
}

unsafe extern "C" fn mock_ctl_void(_state: *mut c_void, _req: i32) -> i32 {
    0
}

unsafe extern "C" fn mock_encode(
    _state: *mut c_void,
    pcm: *const i16,
    _frame_size: i32,
    data: *mut u8,
    max_bytes: i32,
) -> i32 {
    if max_bytes < 4 || pcm.is_null() || data.is_null() {
        return -1;
    }
    // SAFETY: Reading first PCM sample and writing 4 synthetic payload bytes.
    unsafe {
        let first: i16 = *pcm;
        let bytes: [u8; 2] = first.to_le_bytes();
        *data = 0xAB;
        *data.add(1) = 0xCD;
        *data.add(2) = bytes[0];
        *data.add(3) = bytes[1];
    }
    4
}

unsafe extern "C" fn mock_dec_init(_state: *mut c_void, _fs: i32, _ch: i32) -> i32 {
    0
}

unsafe extern "C" fn mock_decode(
    _state: *mut c_void,
    data: *const u8,
    len: i32,
    pcm: *mut i16,
    frame_size: i32,
    decode_fec: i32,
) -> i32 {
    let count: usize = usize::try_from(frame_size).unwrap_or(0);
    let fill_val: i16 = if data.is_null() || len == 0 {
        // PLC marker amplitude (`10_000`) so fade-out attenuation can be asserted precisely.
        10_000
    } else if decode_fec != 0 {
        4_321
    } else if len >= 4 {
        // SAFETY: `data` has at least 4 readable bytes when `len >= 4`.
        unsafe { i16::from_le_bytes([*data.add(2), *data.add(3)]) }
    } else {
        1_000
    };
    // SAFETY: Caller provides `count` writable `i16` samples at `pcm`.
    let out: &mut [i16] = unsafe { std::slice::from_raw_parts_mut(pcm, count) };
    out.fill(fill_val);
    frame_size
}

unsafe extern "C" fn mock_version() -> *const c_char {
    c"mock-libopus-1.1".as_ptr()
}

fn mock_opus_table() -> OpusApiTable {
    OpusApiTable {
        encoder_get_size: mock_get_size,
        encoder_init: mock_enc_init,
        encoder_ctl_set_int: mock_enc_ctl_set,
        encoder_ctl_void: mock_ctl_void,
        encode: mock_encode,
        decoder_get_size: mock_get_size,
        decoder_init: mock_dec_init,
        decoder_ctl_void: mock_ctl_void,
        decode: mock_decode,
        get_version_string: mock_version,
    }
}

#[test]
fn seq_diff_handles_u16_wraparound() {
    assert_eq!(seq_diff(0, 65_535), 1);
    assert_eq!(seq_diff(1, 65_534), 3);
    assert_eq!(seq_diff(65_535, 0), -1);
    assert_eq!(seq_diff(100, 100), 0);
}

#[test]
fn playout_accumulator_caps_burst_at_100ms() {
    let mut accum: u64 = 0;
    assert_eq!(advance_playout_accumulator(&mut accum, 15), 0);
    assert_eq!(accum, 15);
    assert_eq!(advance_playout_accumulator(&mut accum, 10), 1);
    assert_eq!(accum, 5);

    // Huge 600 ms frame hitch must cap at 5 frames (100 ms)
    let mut hitch_accum: u64 = 0;
    assert_eq!(
        advance_playout_accumulator(&mut hitch_accum, 600),
        JITTER_MAX_PLAYOUT_FRAMES_PER_TICK
    );
    assert_eq!(hitch_accum, 0);
}

#[test]
fn jitter_buffer_reorders_wraps_recovers_fec_and_bounds_plc() {
    let mut jb: SpeakerJitterBuffer = SpeakerJitterBuffer::new();
    assert_eq!(jb.target_frames(), 3);

    // Push out-of-order across u16 wrap: 65535, 65534, 1 (skipping 0, with FEC on 1)
    jb.push_packet(65_535, FLAG_HAS_FEC, vec![1, 2, 3], 20);
    assert_eq!(jb.pop_playout_action(), PlayoutAction::Silence);
    jb.push_packet(65_534, FLAG_HAS_FEC, vec![4, 5, 6], 40);
    assert_eq!(jb.pop_playout_action(), PlayoutAction::Silence);
    jb.push_packet(1, FLAG_HAS_FEC, vec![7, 8, 9], 60);

    // Now 3 packets buffered -> talkspurt starts at 65534
    assert_eq!(
        jb.pop_playout_action(),
        PlayoutAction::DecodeFrame {
            payload: vec![4, 5, 6]
        }
    );
    assert_eq!(
        jb.pop_playout_action(),
        PlayoutAction::DecodeFrame {
            payload: vec![1, 2, 3]
        }
    );
    // Packet 0 is missing, packet 1 has FLAG_HAS_FEC -> 1-packet FEC recovery!
    assert_eq!(
        jb.pop_playout_action(),
        PlayoutAction::DecodeFec {
            next_payload: vec![7, 8, 9]
        }
    );
    // Next pop plays packet 1 normally
    assert_eq!(
        jb.pop_playout_action(),
        PlayoutAction::DecodeFrame {
            payload: vec![7, 8, 9]
        }
    );

    // Missing packets 2, 3, 4 -> strictly bounded 3 PLC frames, then silence!
    assert_eq!(
        jb.pop_playout_action(),
        PlayoutAction::Plc { loss_streak: 1 }
    );
    assert_eq!(
        jb.pop_playout_action(),
        PlayoutAction::Plc { loss_streak: 2 }
    );
    assert_eq!(
        jb.pop_playout_action(),
        PlayoutAction::Plc { loss_streak: 3 }
    );
    assert_eq!(jb.pop_playout_action(), PlayoutAction::Silence);
    assert!(!jb.is_in_talkspurt());
    assert!(jb.target_frames() <= JITTER_MAX_TARGET_FRAMES);
    assert!(jb.target_frames() >= JITTER_MIN_TARGET_FRAMES);
}

#[test]
fn jitter_buffer_honors_end_of_talkspurt_and_overflow_cap() {
    let mut jb: SpeakerJitterBuffer = SpeakerJitterBuffer::new();
    jb.push_packet(10, 0, vec![0xAA], 10);
    jb.push_packet(11, FLAG_END_OF_TALKSPURT, Vec::new(), 30);

    // End marker allows short talkspurt to start immediately without waiting for target_frames
    assert_eq!(
        jb.pop_playout_action(),
        PlayoutAction::DecodeFrame {
            payload: vec![0xAA]
        }
    );
    assert_eq!(jb.pop_playout_action(), PlayoutAction::Silence);
    assert!(!jb.is_in_talkspurt());

    // Pushing more than JITTER_MAX_BUFFERED_FRAMES enforces hard queue cap
    for seq in 0..25u16 {
        jb.push_packet(
            seq,
            0,
            vec![u8::try_from(seq).unwrap()],
            u64::from(seq) * 20,
        );
    }
    assert_eq!(jb.buffered_len(), JITTER_MAX_BUFFERED_FRAMES);
    assert!(jb.stats().overflow_drops > 0);
}

#[test]
fn parse_sockaddr_storage_validates_ipv4_and_ipv6() {
    let mut v4: [u8; SOCKADDR_STORAGE_SIZE] = [0u8; SOCKADDR_STORAGE_SIZE];
    v4[0..2].copy_from_slice(&SOCKADDR_AF_INET.to_le_bytes());
    v4[2..4].copy_from_slice(&7777u16.to_be_bytes());
    v4[4..8].copy_from_slice(&[127, 0, 0, 1]);
    v4[32] = 0xFF; // trailing garbage must be normalized to 0
    let (norm_v4, len_v4): ([u8; SOCKADDR_STORAGE_SIZE], i32) =
        parse_sockaddr_storage(&v4).expect("valid IPv4 sockaddr");
    assert_eq!(len_v4, SOCKADDR_INET_LEN);
    assert_eq!(norm_v4[32], 0);

    let mut v6: [u8; SOCKADDR_STORAGE_SIZE] = [0u8; SOCKADDR_STORAGE_SIZE];
    v6[0..2].copy_from_slice(&SOCKADDR_AF_INET6.to_le_bytes());
    v6[2..4].copy_from_slice(&7895u16.to_be_bytes());
    let (_norm_v6, len_v6): ([u8; SOCKADDR_STORAGE_SIZE], i32) =
        parse_sockaddr_storage(&v6).expect("valid IPv6 sockaddr");
    assert_eq!(len_v6, SOCKADDR_INET6_LEN);

    // Zero port rejected
    v4[2..4].copy_from_slice(&0u16.to_be_bytes());
    assert!(parse_sockaddr_storage(&v4).is_none());
}

#[test]
fn voice_rx_state_and_mock_opus_deliver_decoded_fec_and_faded_plc() {
    let api: OpusApiTable = mock_opus_table();
    assert_eq!(api.version_string(), "mock-libopus-1.1");

    let mut enc: OpusEncoder = OpusEncoder::new_voice_48k_mono(api).unwrap();
    let mut dec: OpusDecoder = OpusDecoder::new_voice_48k_mono(api).unwrap();
    let mut enc_out: [u8; 64] = [0u8; 64];
    let pcm_in: [i16; OPUS_FRAME_SAMPLES_20MS] = [1234; OPUS_FRAME_SAMPLES_20MS];
    let enc_len: usize = enc.encode_20ms(&pcm_in, &mut enc_out).unwrap();
    assert_eq!(enc_len, 4);

    let mut pcm_out: [i16; OPUS_FRAME_SAMPLES_20MS] = [0; OPUS_FRAME_SAMPLES_20MS];
    let dec_len: usize = dec
        .decode_20ms(Some(&enc_out[..enc_len]), &mut pcm_out, false)
        .unwrap();
    assert_eq!(dec_len, OPUS_FRAME_SAMPLES_20MS);
    assert_eq!(pcm_out[0], 1234);

    // End-to-end VoiceRxState with PeerMap + FEC + PLC fade-out
    let mut rx: VoiceRxState = VoiceRxState::new(Some(api));
    rx.apply_peer_map(&[PeerMapEntry {
        peer_slot: 2,
        uid: "76561198000000001|0002abc".to_string(),
    }]);
    assert_eq!(rx.peer_count(), 1);
    assert_eq!(rx.peer_uid(2), Some("76561198000000001|0002abc"));

    let t0: Instant = Instant::now();
    rx.push_voice_frame(2, FLAG_HAS_FEC, 0, enc_out[..4].to_vec(), t0);
    rx.push_voice_frame(
        2,
        FLAG_HAS_FEC,
        2,
        enc_out[..4].to_vec(),
        t0 + Duration::from_millis(40),
    );
    rx.push_voice_frame(
        2,
        FLAG_HAS_FEC,
        3,
        enc_out[..4].to_vec(),
        t0 + Duration::from_millis(60),
    );

    let mut out_frames: Vec<DecodedPlayoutFrame> = Vec::new();
    // Initialize playout clock at t0, then step +100 ms (5 frames = normal, FEC, normal, normal, PLC#1)
    rx.drain_playout_frames(t0, &mut out_frames);
    assert!(out_frames.is_empty());

    rx.drain_playout_frames(t0 + Duration::from_millis(100), &mut out_frames);
    assert_eq!(out_frames.len(), 5);
    let expected_uid: Vec<u16> = encode_uid_utf16_nul("76561198000000001|0002abc");
    assert_eq!(out_frames[0].uid_utf16_nul, expected_uid);
    assert_eq!(out_frames[0].pcm_i16[0], 1234);
    assert_eq!(out_frames[1].pcm_i16[0], 4_321); // In-Band FEC recovery for lost seq 1
    assert_eq!(out_frames[2].pcm_i16[0], 1234);
    assert_eq!(out_frames[3].pcm_i16[0], 1234);
    assert_eq!(out_frames[4].pcm_i16[0], 10_000); // PLC#1 gain = 1.0

    // Next 40 ms -> PLC#2 (0.70 fade) and PLC#3 (0.35 fade)
    out_frames.clear();
    rx.drain_playout_frames(t0 + Duration::from_millis(140), &mut out_frames);
    assert_eq!(out_frames.len(), 2);
    assert!((i32::from(out_frames[0].pcm_i16[0]) - 7_000).abs() <= 5);
    assert!((i32::from(out_frames[1].pcm_i16[0]) - 3_500).abs() <= 5);

    // Direct helper check
    let mut direct_plc: [i16; OPUS_FRAME_SAMPLES_20MS] = [10_000; OPUS_FRAME_SAMPLES_20MS];
    apply_plc_fade_in_place(&mut direct_plc, 3);
    assert!((plc_fade_gain(3) - 0.35).abs() < 1.0e-5);
    assert!((i32::from(direct_plc[0]) - 3_500).abs() <= 5);

    // PeerMap pruning removes vanished slots
    rx.apply_peer_map(&[]);
    assert_eq!(rx.peer_count(), 0);
}
