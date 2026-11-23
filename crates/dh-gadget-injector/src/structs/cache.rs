use std::path::PathBuf;

/// Manages the persistent SHA-256 verified Frida Gadget DLL cache and per-round
/// isolated execution directories (using NTFS hard links when on the same volume).
#[derive(Debug, Clone)]
pub struct GadgetCache {
    pub(crate) root_dir: PathBuf,
    pub(crate) cached_dll_path: PathBuf,
}

/// RAII owner for a single match round's `run_<pid>_<round>/` staging directory.
#[derive(Debug)]
pub struct RoundTempDir {
    pub(crate) dir: PathBuf,
    pub(crate) gadget_dll_path: PathBuf,
    pub(crate) config_path: PathBuf,
    pub(crate) bundle_js_path: PathBuf,
    pub(crate) cleaned: bool,
}
