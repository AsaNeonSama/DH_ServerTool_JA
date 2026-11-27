/// Single verified in-place byte patch in the client executable image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytePatch {
    /// Human-readable identifier of the patch site.
    pub name: &'static str,
    /// Relative virtual address from `DreadHunger-Win64-Shipping.exe` base.
    pub rva: usize,
    /// Exact original machine-code bytes required before patching.
    pub expected: &'static [u8],
    /// Replacement machine-code bytes (must match `expected.len()`).
    pub replacement: &'static [u8],
}

/// Verified `.rdata` vtable slot replacement specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VtablePatchSpec {
    /// Human-readable identifier of the vtable patch.
    pub name: &'static str,
    /// Relative virtual address of the 8-byte vtable slot.
    pub slot_rva: usize,
    /// Expected original function target RVA stored inside the vtable slot.
    pub expected_target_rva: usize,
}

/// Verified function-entry detour specification (`5`-byte `E9 rel32` into nearby `RX` stub).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetourSpec {
    /// Human-readable identifier of the detour site.
    pub name: &'static str,
    /// Relative virtual address of the hooked function entry point.
    pub rva: usize,
    /// Exact original prologue bytes overwritten at `rva` (must be `>= 5` bytes and free of
    /// RIP-relative operands).
    pub expected_prologue: &'static [u8],
}

/// Specification for the mid-function PCM redirect hook inside
/// `FVoiceEngineImpl::SubmitRemoteVoiceData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmitRemoteVoiceSpec {
    /// Human-readable identifier of the hook site.
    pub name: &'static str,
    /// RVA of the instruction right after `FVoiceDecoderOpus::Decode` succeeds (`0x927C0C`).
    pub hook_rva: usize,
    /// Exact expected `12` bytes overwritten at `hook_rva`.
    pub expected_bytes: &'static [u8],
    /// RVA of the clean register-restoring epilogue (`0x92824B`).
    pub epilogue_rva: usize,
}

/// Specification for the zero-overhead naked guard detour on
/// `FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EosRecvGuardSpec {
    /// Human-readable identifier of the guard site.
    pub name: &'static str,
    /// RVA of `FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered` (`0xA2DC70`).
    pub rva: usize,
    /// Exact expected `10` prologue bytes (`mov rax, rsp; sub rsp, 0x88`).
    pub expected_prologue: [u8; 10],
}

/// Outcome of attempting to verify and apply a single patch or detour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOutcome {
    /// Original bytes matched and replacement was written.
    Applied,
    /// Target memory already contained the replacement bytes.
    AlreadyApplied,
    /// RVA or byte length fell outside the mapped image bounds.
    OutOfBounds,
    /// Original bytes did not match expected signature; safely skipped.
    SignatureMismatch,
    /// Page protection or nearby trampoline allocation failed.
    MemoryError,
}

/// Aggregate result of applying all client voice patches at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchReport {
    /// Whether the host process matched `DreadHunger-Win64-Shipping.exe`.
    pub target_matched: bool,
    /// Number of patches newly applied.
    pub applied: usize,
    /// Number of patches already present.
    pub already_applied: usize,
    /// Number of patches skipped due to mismatch, bounds, or memory failure.
    pub skipped: usize,
}

/// Error returned while encoding a jump or trampoline stub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrampolineError {
    /// Computed relative displacement does not fit in a signed 32-bit offset.
    DisplacementOutOfRange,
    /// Address arithmetic overflowed `usize`.
    AddressOverflow,
    /// Requested stub exceeded the remaining capacity of the trampoline page.
    BufferTooSmall,
}

/// Binding pairing a verified `DetourSpec` with its Rust detour entry address and original-stub
/// registration callback.
#[derive(Clone, Copy)]
pub struct StandardDetourBinding {
    /// Verified hook-site specification.
    pub spec: &'static DetourSpec,
    /// Address of the Rust `extern "C"` detour handler function.
    pub handler_addr: usize,
    /// Callback recording the allocated `RX` original-call trampoline address.
    pub register_orig: fn(usize),
}

/// Deferred code-site patch queued until the shared nearby trampoline page becomes `RX`.
pub struct PendingSitePatch {
    /// Human-readable name for diagnostic logging.
    pub name: &'static str,
    /// Virtual address of the executable code site to overwrite.
    pub site_addr: usize,
    /// Exact replacement bytes (`E9 rel32` + optional NOP padding).
    pub patch_bytes: Vec<u8>,
}

/// Bump allocator accumulating all client voice stubs inside one nearby `RW` -> `RX` page.
pub struct TrampolinePageBuilder<'a> {
    /// Loaded base address of `DreadHunger-Win64-Shipping.exe`.
    pub module_base: usize,
    /// Parsed PE `SizeOfImage` for bounds checking.
    pub image_size: usize,
    /// Base virtual address of the allocated nearby trampoline page.
    pub page_addr: usize,
    /// Writable view over the allocated trampoline page prior to `PAGE_EXECUTE_READ`.
    pub page_slice: &'a mut [u8],
    /// Current bump offset into `page_slice`.
    pub cursor: usize,
    /// Verified site patches ready to write once the trampoline page is flipped to `RX`.
    pub pending_patches: Vec<PendingSitePatch>,
}
