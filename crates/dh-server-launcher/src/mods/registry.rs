use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

pub use crate::consts::mods::{
    CONSOLE_PATCH_ID, CONSOLE_PATCH_PORT_TOKEN, CONTENT_MOD_ASSET_ROOT_TOKEN,
    DEFAULT_PATCH_SCRIPTS, SELECTABLE_CONTENT_MOD_OPTIONS, SELECTABLE_MOD_OPTIONS,
};
use crate::languages;
use crate::mods::content_assets::{extract_content_mod_assets, format_js_single_quoted_path};
pub use crate::structs::mods::{
    ContentModAssetBundle, ExtractedServerDll, ModGroup, ModKind, ModMenuOption, PatchScript,
    ResolvedPatchScript, ServerBuiltinDllId,
};

#[must_use]
pub fn find_option_in_list(
    options: &'static [ModMenuOption],
    number: u32,
) -> Option<&'static ModMenuOption> {
    options
        .iter()
        .find(|item: &&ModMenuOption| item.number == number)
}

#[must_use]
pub fn find_option_by_number(number: u32) -> Option<&'static ModMenuOption> {
    find_option_in_list(SELECTABLE_MOD_OPTIONS, number)
}

#[must_use]
pub fn find_content_option_by_number(number: u32) -> Option<&'static ModMenuOption> {
    find_option_in_list(SELECTABLE_CONTENT_MOD_OPTIONS, number)
}

#[must_use]
pub fn are_mod_options_conflicting(a: &ModMenuOption, b: &ModMenuOption) -> bool {
    if a.id == b.id {
        return false;
    }
    a.conflicts_with.contains(&b.id) || b.conflicts_with.contains(&a.id)
}

#[must_use]
pub fn find_conflicting_selected_in_list(
    options: &'static [ModMenuOption],
    candidate: &ModMenuOption,
    selected_numbers: &[u32],
) -> Vec<&'static ModMenuOption> {
    let mut conflicts: Vec<&'static ModMenuOption> = Vec::new();
    for &num in selected_numbers {
        if let Some(selected) = find_option_in_list(options, num) {
            if are_mod_options_conflicting(candidate, selected) {
                conflicts.push(selected);
            }
        }
    }
    conflicts
}

#[must_use]
pub fn find_conflicting_selected_options(
    candidate: &ModMenuOption,
    selected_numbers: &[u32],
) -> Vec<&'static ModMenuOption> {
    find_conflicting_selected_in_list(SELECTABLE_MOD_OPTIONS, candidate, selected_numbers)
}

#[must_use]
pub fn get_conflict_pairs_in_list(
    options: &'static [ModMenuOption],
) -> Vec<(&'static ModMenuOption, &'static ModMenuOption)> {
    let mut seen_pairs: HashSet<(&'static str, &'static str)> = HashSet::new();
    let mut pairs: Vec<(&'static ModMenuOption, &'static ModMenuOption)> = Vec::new();

    for (i, a) in options.iter().enumerate() {
        for b in options.iter().skip(i + 1) {
            if !are_mod_options_conflicting(a, b) {
                continue;
            }
            if seen_pairs.insert((a.id, b.id)) {
                pairs.push((a, b));
            }
        }
    }

    pairs
}

#[must_use]
pub fn get_mod_conflict_pairs() -> Vec<(&'static ModMenuOption, &'static ModMenuOption)> {
    get_conflict_pairs_in_list(SELECTABLE_MOD_OPTIONS)
}

fn append_selected_options(
    options: &'static [ModMenuOption],
    selected_numbers: &[u32],
    asset_roots: Option<&BTreeMap<&'static str, PathBuf>>,
    seen: &mut HashSet<&'static str>,
    accepted_options: &mut Vec<&'static ModMenuOption>,
    result: &mut Vec<ResolvedPatchScript>,
) {
    for &num in selected_numbers {
        let Some(opt): Option<&'static ModMenuOption> = find_option_in_list(options, num) else {
            continue;
        };
        let has_conflict: bool = accepted_options
            .iter()
            .any(|existing: &&ModMenuOption| are_mod_options_conflicting(existing, opt));
        if has_conflict {
            continue;
        }
        accepted_options.push(opt);
        for patch in opt.scripts {
            if !seen.insert(patch.id) {
                continue;
            }
            let stripped: &str = patch
                .source
                .strip_prefix('\u{feff}')
                .unwrap_or(patch.source);
            let resolved_source: String = match (patch.asset_bundle, asset_roots) {
                (Some(bundle), Some(roots)) => match roots.get(bundle.mod_id) {
                    Some(dir) => {
                        let escaped: String = format_js_single_quoted_path(dir);
                        stripped.replace(CONTENT_MOD_ASSET_ROOT_TOKEN, &escaped)
                    }
                    None => stripped.to_string(),
                },
                _ => stripped.to_string(),
            };
            result.push(ResolvedPatchScript {
                id: patch.id.to_string(),
                file_name: patch.file_name.to_string(),
                label: languages::mod_label(patch.label_code).to_string(),
                group: patch.group,
                source: resolved_source,
            });
        }
    }
}

/// Resolves the ordered list of scripts to inject for a match without touching the filesystem:
/// 5 default scripts first, followed by selected Content MODs (with optional asset root
/// substitution), followed by selected server-only JS MODs.
#[must_use]
pub fn build_injection_plan_with_content_mods(
    selected_content_numbers: &[u32],
    selected_option_numbers: &[u32],
    console_patch_port: u16,
    asset_roots: Option<&BTreeMap<&'static str, PathBuf>>,
) -> Vec<ResolvedPatchScript> {
    let mut seen: HashSet<&'static str> = HashSet::new();
    let mut accepted_options: Vec<&'static ModMenuOption> = Vec::new();
    let mut result: Vec<ResolvedPatchScript> = Vec::new();

    let port_str: String = console_patch_port.to_string();

    for patch in DEFAULT_PATCH_SCRIPTS {
        let stripped: &str = patch
            .source
            .strip_prefix('\u{feff}')
            .unwrap_or(patch.source);
        let resolved_source: String = if patch.id == CONSOLE_PATCH_ID {
            stripped.replace(CONSOLE_PATCH_PORT_TOKEN, &port_str)
        } else {
            stripped.to_string()
        };
        seen.insert(patch.id);
        result.push(ResolvedPatchScript {
            id: patch.id.to_string(),
            file_name: patch.file_name.to_string(),
            label: languages::mod_label(patch.label_code).to_string(),
            group: patch.group,
            source: resolved_source,
        });
    }

    append_selected_options(
        SELECTABLE_CONTENT_MOD_OPTIONS,
        selected_content_numbers,
        asset_roots,
        &mut seen,
        &mut accepted_options,
        &mut result,
    );
    append_selected_options(
        SELECTABLE_MOD_OPTIONS,
        selected_option_numbers,
        asset_roots,
        &mut seen,
        &mut accepted_options,
        &mut result,
    );

    result
}

/// Convenience helper for server-only JS MOD selection (no Content MODs selected).
#[must_use]
pub fn build_injection_plan(
    selected_option_numbers: &[u32],
    console_patch_port: u16,
) -> Vec<ResolvedPatchScript> {
    build_injection_plan_with_content_mods(&[], selected_option_numbers, console_patch_port, None)
}

/// Extracts any embedded server-side asset bundles required by selected Content MODs into the
/// verified cache directory and returns the complete ready-to-inject script plan.
pub fn prepare_injection_plan(
    selected_content_numbers: &[u32],
    selected_option_numbers: &[u32],
    console_patch_port: u16,
    override_cache_root: Option<&Path>,
) -> Result<Vec<ResolvedPatchScript>, String> {
    let mut asset_roots: BTreeMap<&'static str, PathBuf> = BTreeMap::new();

    for &num in selected_content_numbers {
        let Some(opt): Option<&'static ModMenuOption> = find_content_option_by_number(num) else {
            continue;
        };
        for patch in opt.scripts {
            if let Some(bundle) = patch.asset_bundle {
                if !asset_roots.contains_key(bundle.mod_id) {
                    let extracted_dir: PathBuf =
                        extract_content_mod_assets(bundle, override_cache_root)?;
                    asset_roots.insert(bundle.mod_id, extracted_dir);
                }
            }
        }
    }

    Ok(build_injection_plan_with_content_mods(
        selected_content_numbers,
        selected_option_numbers,
        console_patch_port,
        Some(&asset_roots),
    ))
}
