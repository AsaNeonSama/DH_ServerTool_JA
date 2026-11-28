//! Types for binary detour patching and hook initialization reporting.

/// Result of attempting to encode or install a trampoline detour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetourError {
    /// Address arithmetic overflowed `usize` / `i128`.
    AddressOverflow,
    /// Relative branch displacement exceeded signed 32-bit range.
    DisplacementOutOfRange,
    /// Expected prologue bytes did not match target memory.
    SignatureMismatch,
    /// Target RVA lies outside the PE image bounds.
    OutOfBounds,
    /// Virtual memory allocation or protection change failed.
    MemoryError,
}

/// Summary of hook installation performed during `DllMain` / initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookInstallReport {
    /// `true` if the host process filename matched `DreadHungerServer-Win64-Shipping.exe`.
    pub target_matched: bool,
    /// Number of detours installed (`3` on success: `RecvFrom`, `Close`, `ReceivedPacket`).
    pub hooks_installed: u8,
    /// Error code if installation failed on a matched target.
    pub error: Option<DetourError>,
}
