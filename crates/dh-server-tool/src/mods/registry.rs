use std::collections::HashSet;

pub use crate::consts::mods::{
    CONSOLE_PATCH_ID, CONSOLE_PATCH_PORT_TOKEN, DEFAULT_PATCH_SCRIPTS, SELECTABLE_MOD_OPTIONS,
};
use crate::languages;
pub use crate::structs::mods::{
    ModGroup, ModKind, ModMenuOption, PatchScript, ResolvedPatchScript,
};

#[must_use]
pub fn find_option_by_number(number: u32) -> Option<&'static ModMenuOption> {
    SELECTABLE_MOD_OPTIONS
        .iter()
        .find(|item: &&ModMenuOption| item.number == number)
}

#[must_use]
pub fn are_mod_options_conflicting(a: &ModMenuOption, b: &ModMenuOption) -> bool {
    if a.id == b.id {
        return false;
    }
    a.conflicts_with.contains(&b.id) || b.conflicts_with.contains(&a.id)
}

#[must_use]
pub fn find_conflicting_selected_options(
    candidate: &ModMenuOption,
    selected_numbers: &[u32],
) -> Vec<&'static ModMenuOption> {
    let mut conflicts: Vec<&'static ModMenuOption> = Vec::new();
    for &num in selected_numbers {
        if let Some(selected) = find_option_by_number(num) {
            if are_mod_options_conflicting(candidate, selected) {
                conflicts.push(selected);
            }
        }
    }
    conflicts
}

#[must_use]
pub fn get_mod_conflict_pairs() -> Vec<(&'static ModMenuOption, &'static ModMenuOption)> {
    let mut seen_pairs: HashSet<(&'static str, &'static str)> = HashSet::new();
    let mut pairs: Vec<(&'static ModMenuOption, &'static ModMenuOption)> = Vec::new();

    for (i, a) in SELECTABLE_MOD_OPTIONS.iter().enumerate() {
        for b in SELECTABLE_MOD_OPTIONS.iter().skip(i + 1) {
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

/// Resolves the ordered list of scripts to inject for a match:
/// 4 default scripts first (applying `__CONSOLE_PATCH_PORT__` substitution to the console bridge),
/// followed by all non-conflicting selected optional MODs (expanding Bone Dagger into 3 scripts).
#[must_use]
pub fn build_injection_plan(
    selected_option_numbers: &[u32],
    console_patch_port: u16,
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

    for &num in selected_option_numbers {
        let Some(opt): Option<&'static ModMenuOption> = find_option_by_number(num) else {
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
            result.push(ResolvedPatchScript {
                id: patch.id.to_string(),
                file_name: patch.file_name.to_string(),
                label: languages::mod_label(patch.label_code).to_string(),
                group: patch.group,
                source: stripped.to_string(),
            });
        }
    }

    result
}
