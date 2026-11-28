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
    pub conflicts_with: &'static [&'static str],
}
