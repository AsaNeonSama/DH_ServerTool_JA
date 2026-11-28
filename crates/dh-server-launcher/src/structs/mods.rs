use std::path::PathBuf;

use crate::structs::languages::ModLabelCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModGroup {
    Default,
    ContentMod,
    Single,
    BoneDagger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModKind {
    Single,
    Mode,
}

/// Compile-time embedded server-side native DLLs injected at OEP alongside Frida JS scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServerBuiltinDllId {
    VoiceServer,
}

/// Verified cached extraction of a server-side OEP DLL ready for `InjectLaunchRequest::dlls`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedServerDll {
    pub id: ServerBuiltinDllId,
    pub file_name: String,
    pub label: String,
    pub path: PathBuf,
    pub sha256: String,
    pub raw_bytes: usize,
    pub reused_cache: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ContentModAssetBundle {
    pub mod_id: &'static str,
    pub archive_zst: &'static [u8],
    pub archive_sha256: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct PatchScript {
    pub id: &'static str,
    pub file_name: &'static str,
    pub label_code: ModLabelCode,
    pub group: ModGroup,
    pub source: &'static str,
    pub asset_bundle: Option<&'static ContentModAssetBundle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPatchScript {
    pub id: String,
    pub file_name: String,
    pub label: String,
    pub group: ModGroup,
    pub source: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ModMenuOption {
    pub id: &'static str,
    pub number: u32,
    pub label_code: ModLabelCode,
    pub kind: ModKind,
    pub scripts: &'static [PatchScript],
    pub dlls: &'static [ServerBuiltinDllId],
    pub conflicts_with: &'static [&'static str],
}
