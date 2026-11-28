/// WASAPI shared-mode buffer duration in 100-nanosecond `REFERENCE_TIME` units (`40 ms`).
pub const WASAPI_BUFFER_DURATION_HNS: i64 = 400_000;

/// Event wait timeout in milliseconds on the WASAPI capture thread.
pub const WASAPI_CAPTURE_WAIT_MS: u32 = 50;

/// Polling cadence in milliseconds on the WASAPI loopback render-reference thread (`10 ms`).
pub const WASAPI_LOOPBACK_POLL_MS: u32 = 10;

/// Sleep duration in milliseconds before reopening a WASAPI endpoint after hot-plug invalidation.
pub const WASAPI_RECONNECT_BACKOFF_MS: u32 = 250;

/// Capacity (in `10 ms` frames of `480` samples) of the loopback render-reference ring buffer
/// (`16` frames = `160 ms` headroom between loopback and capture threads).
pub const WASAPI_RENDER_RING_CAPACITY_FRAMES: usize = 16;

/// Maximum mono samples buffered inside one WASAPI capture accumulator before flushing (`4800` = `100 ms`).
pub const WASAPI_MAX_ACCUMULATOR_SAMPLES: usize = 4_800;

/// Standard `WAVEFORMATEX::wFormatTag` values.
pub const WAVE_FORMAT_PCM_TAG: u16 = 0x0001;
pub const WAVE_FORMAT_IEEE_FLOAT_TAG: u16 = 0x0003;
pub const WAVE_FORMAT_EXTENSIBLE_TAG: u16 = 0xFFFE;

/// `KSDATAFORMAT_SUBTYPE_PCM` GUID (`00000001-0000-0010-8000-00aa00389b71`).
pub const SUBTYPE_PCM_DATA1: u32 = 0x0000_0001;
/// `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT` GUID (`00000003-0000-0010-8000-00aa00389b71`).
pub const SUBTYPE_IEEE_FLOAT_DATA1: u32 = 0x0000_0003;

/// WASAPI hot-plug / service-restart HRESULT values that trigger automatic endpoint recovery.
pub const HRESULT_AUDCLNT_E_DEVICE_INVALIDATED: i32 = 0x8889_0004_u32.cast_signed();
pub const HRESULT_AUDCLNT_E_SERVICE_NOT_RUNNING: i32 = 0x8889_0010_u32.cast_signed();
pub const HRESULT_AUDCLNT_E_RESOURCES_INVALIDATED: i32 = 0x8889_0026_u32.cast_signed();
