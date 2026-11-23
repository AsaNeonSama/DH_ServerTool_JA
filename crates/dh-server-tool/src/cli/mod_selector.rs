use tokio_util::sync::CancellationToken;

use crate::colors;
use crate::messages::ja::cli as cli_msg;
use crate::mods::{
    find_conflicting_selected_options, find_option_by_number, get_mod_conflict_pairs, ModKind,
    ModMenuOption, PatchScript, DEFAULT_PATCH_SCRIPTS, SELECTABLE_MOD_OPTIONS,
};
use crate::terminal::{pad_display_width, PromptError, Terminal};

pub const MOD_MENU_COLS: usize = 4;
pub const MOD_MENU_COL_WIDTH: usize = 27;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModSelectionStepResult {
    pub added_labels: Vec<&'static str>,
    pub duplicate_labels: Vec<&'static str>,
    pub conflict_notices: Vec<String>,
    pub invalid_tokens: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModSelectionCommand {
    Confirm,
    Cleared,
    EmptyInvalid,
    Processed(ModSelectionStepResult),
}

/// Formats the 4-column selectable MOD menu lines and the bidirectional conflict legend.
#[must_use]
pub fn build_mod_menu_lines() -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let total: usize = SELECTABLE_MOD_OPTIONS.len();
    let rows: usize = total.div_ceil(MOD_MENU_COLS);

    for r in 0..rows {
        let mut line: String = String::from(" ");
        for c in 0..MOD_MENU_COLS {
            let idx: usize = c * rows + r;
            let Some(opt): Option<&ModMenuOption> = SELECTABLE_MOD_OPTIONS.get(idx) else {
                continue;
            };
            let suffix: &str = if opt.kind == ModKind::Mode {
                cli_msg::MOD_MODE_SUFFIX
            } else {
                ""
            };
            let cell: String = format!("{}:{}{suffix}", opt.number, opt.label);
            let has_next_col: bool = (c + 1) * rows + r < total;
            if has_next_col {
                line.push_str(&pad_display_width(&cell, MOD_MENU_COL_WIDTH));
            } else {
                line.push_str(&cell);
            }
        }
        lines.push(format!("{}{line}", colors::CYAN));
    }

    let conflict_pairs: Vec<(&'static ModMenuOption, &'static ModMenuOption)> =
        get_mod_conflict_pairs();
    lines.push(String::new());
    if !conflict_pairs.is_empty() {
        let pair_text: String = conflict_pairs
            .iter()
            .map(
                |(a, b): &(&'static ModMenuOption, &'static ModMenuOption)| {
                    format!("{}:{} <-> {}:{}", a.number, a.label, b.number, b.label)
                },
            )
            .collect::<Vec<String>>()
            .join(" / ");
        lines.push(format!(
            "{} * {} {pair_text}",
            colors::LIGHT_BLACK,
            cli_msg::MOD_CONFLICT_LEGEND_PREFIX,
        ));
    }

    lines
}

#[must_use]
pub fn format_selected_labels(selected_numbers: &[u32]) -> String {
    selected_numbers
        .iter()
        .map(|&n: &u32| {
            find_option_by_number(n).map_or_else(
                || n.to_string(),
                |item: &ModMenuOption| item.label.to_string(),
            )
        })
        .collect::<Vec<String>>()
        .join(", ")
}

/// Splits batch input on ASCII comma `,`, fullwidth comma `，`, and Japanese ideographic comma `、`.
#[must_use]
pub fn split_mod_tokens(raw_choice: &str) -> Vec<&str> {
    raw_choice
        .trim()
        .split([',', '，', '、'])
        .map(str::trim)
        .filter(|part: &&str| !part.is_empty())
        .collect()
}

/// Applies one line of user input (`Y`, `C`, or comma-separated MOD numbers) to `selected_numbers`.
pub fn apply_mod_selection_input(
    selected_numbers: &mut Vec<u32>,
    raw_choice: &str,
) -> ModSelectionCommand {
    let trimmed: &str = raw_choice.trim();
    if trimmed.eq_ignore_ascii_case("y") {
        return ModSelectionCommand::Confirm;
    }
    if trimmed.eq_ignore_ascii_case("c") {
        selected_numbers.clear();
        return ModSelectionCommand::Cleared;
    }

    let tokens: Vec<&str> = split_mod_tokens(trimmed);
    if tokens.is_empty() {
        return ModSelectionCommand::EmptyInvalid;
    }

    let mut added_labels: Vec<&'static str> = Vec::new();
    let mut duplicate_labels: Vec<&'static str> = Vec::new();
    let mut conflict_notices: Vec<String> = Vec::new();
    let mut invalid_tokens: Vec<String> = Vec::new();

    for token in tokens {
        let matched: Option<&'static ModMenuOption> =
            token.parse::<u32>().ok().and_then(find_option_by_number);

        let Some(opt): Option<&'static ModMenuOption> = matched else {
            invalid_tokens.push(token.to_string());
            continue;
        };

        if selected_numbers.contains(&opt.number) {
            if !duplicate_labels.contains(&opt.label) {
                duplicate_labels.push(opt.label);
            }
            continue;
        }

        let conflicts: Vec<&'static ModMenuOption> =
            find_conflicting_selected_options(opt, selected_numbers);
        if !conflicts.is_empty() {
            let conflict_targets: String = conflicts
                .iter()
                .map(|item: &&ModMenuOption| format!("{}:{}", item.number, item.label))
                .collect::<Vec<String>>()
                .join(", ");
            let notice: String = cli_msg::mod_conflict_detail(
                &format!("{}:{}", opt.number, opt.label),
                &conflict_targets,
            );
            if !conflict_notices.contains(&notice) {
                conflict_notices.push(notice);
            }
            continue;
        }

        selected_numbers.push(opt.number);
        added_labels.push(opt.label);
    }

    ModSelectionCommand::Processed(ModSelectionStepResult {
        added_labels,
        duplicate_labels,
        conflict_notices,
        invalid_tokens,
    })
}

pub async fn prompt_mod_selection(
    terminal: &Terminal,
    cancel: Option<&CancellationToken>,
) -> Result<(Vec<u32>, Vec<String>), PromptError> {
    let default_labels: String = DEFAULT_PATCH_SCRIPTS
        .iter()
        .map(|patch: &PatchScript| patch.label)
        .collect::<Vec<&str>>()
        .join(", ");

    terminal.println(&format!(
        "{} * {} {}{default_labels}",
        colors::LIGHT_CYAN,
        cli_msg::DEFAULT_MODS_LABEL,
        colors::LIGHT_GREEN,
    ));
    terminal.println(&format!(
        "{} {} {}Y/y {}{} {}C/c {}{}",
        colors::LIGHT_YELLOW,
        cli_msg::MOD_SELECTION_GUIDE_BEFORE_Y,
        colors::LIGHT_GREEN,
        colors::LIGHT_YELLOW,
        cli_msg::MOD_SELECTION_GUIDE_BETWEEN_Y_AND_C,
        colors::LIGHT_GREEN,
        colors::LIGHT_YELLOW,
        cli_msg::MOD_SELECTION_GUIDE_AFTER_C,
    ));
    terminal.println_empty();

    for line in build_mod_menu_lines() {
        terminal.println(&line);
    }

    let prompt: String = format!("{} * {} ", colors::LIGHT_CYAN, cli_msg::MOD_CHOICE_PROMPT);
    let mut selected_numbers: Vec<u32> = Vec::new();

    loop {
        let raw_choice: String = terminal.ask(&prompt, cancel).await?;
        match apply_mod_selection_input(&mut selected_numbers, &raw_choice) {
            ModSelectionCommand::Confirm => break,
            ModSelectionCommand::Cleared => {
                terminal.println(&format!(
                    "{} * {}",
                    colors::LIGHT_YELLOW,
                    cli_msg::MOD_SELECTION_CLEARED
                ));
            }
            ModSelectionCommand::EmptyInvalid => {
                terminal.println(&format!(
                    "{} * {}",
                    colors::RED,
                    cli_msg::INVALID_CHOICE_SHORT
                ));
            }
            ModSelectionCommand::Processed(step) => {
                if !step.added_labels.is_empty() {
                    let current_names: String = format_selected_labels(&selected_numbers);
                    terminal.println(&format!(
                        "{} * {} {} {}{}",
                        colors::LIGHT_GREEN,
                        cli_msg::MOD_ADDED_PREFIX,
                        step.added_labels.join(", "),
                        colors::LIGHT_BLACK,
                        cli_msg::mod_current_selection(&current_names),
                    ));
                }
                if !step.duplicate_labels.is_empty() {
                    let current_names: String = format_selected_labels(&selected_numbers);
                    terminal.println(&format!(
                        "{} * {} {} {}{}",
                        colors::LIGHT_YELLOW,
                        cli_msg::MOD_DUPLICATE_PREFIX,
                        step.duplicate_labels.join(", "),
                        colors::LIGHT_BLACK,
                        cli_msg::mod_current_selection(&current_names),
                    ));
                }
                if !step.conflict_notices.is_empty() {
                    terminal.println(&format!(
                        "{} * {} {}",
                        colors::RED,
                        cli_msg::MOD_CONFLICT_SKIPPED_PREFIX,
                        step.conflict_notices.join(" / "),
                    ));
                }
                if !step.invalid_tokens.is_empty() {
                    terminal.println(&format!(
                        "{} * {}",
                        colors::RED,
                        cli_msg::invalid_choice_list(&step.invalid_tokens.join(", ")),
                    ));
                }
            }
        }
    }

    let selected_labels: Vec<String> = selected_numbers
        .iter()
        .map(|&n: &u32| {
            find_option_by_number(n).map_or_else(
                || n.to_string(),
                |item: &ModMenuOption| item.label.to_string(),
            )
        })
        .collect();

    Ok((selected_numbers, selected_labels))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::display::strip_ansi;

    #[test]
    fn batch_input_supports_ascii_fullwidth_and_japanese_commas() {
        let mut selected = Vec::new();
        let res = apply_mod_selection_input(&mut selected, "1, 2，7、13");
        assert_eq!(selected, vec![1, 2, 7, 13]);
        match res {
            ModSelectionCommand::Processed(step) => {
                assert_eq!(step.added_labels.len(), 4);
                assert_eq!(step.duplicate_labels, Vec::<&str>::new());
                assert_eq!(step.conflict_notices, Vec::<String>::new());
                assert_eq!(step.invalid_tokens, Vec::<String>::new());
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let dup = apply_mod_selection_input(&mut selected, "7, 99, abc");
        assert_eq!(selected, vec![1, 2, 7, 13]);
        match dup {
            ModSelectionCommand::Processed(step) => {
                assert_eq!(step.added_labels, Vec::<&str>::new());
                assert_eq!(step.duplicate_labels, vec!["骨の短剣モード"]);
                assert_eq!(step.invalid_tokens, vec!["99", "abc"]);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        assert_eq!(
            apply_mod_selection_input(&mut selected, "C"),
            ModSelectionCommand::Cleared
        );
        assert_eq!(selected, Vec::<u32>::new());
        assert_eq!(
            apply_mod_selection_input(&mut selected, "y"),
            ModSelectionCommand::Confirm
        );
        assert_eq!(
            apply_mod_selection_input(&mut selected, "  , 、 "),
            ModSelectionCommand::EmptyInvalid
        );
    }

    #[test]
    fn enforces_all_four_bidirectional_conflict_rules_in_both_directions() {
        let pairs = [(3u32, 21u32), (4, 22), (12, 14), (21, 22)];
        for (a, b) in pairs {
            let mut sel_fwd = Vec::new();
            let res_fwd = apply_mod_selection_input(&mut sel_fwd, &format!("{a},{b}"));
            assert_eq!(sel_fwd, vec![a], "forward conflict failed for {a}<->{b}");
            match res_fwd {
                ModSelectionCommand::Processed(step) => {
                    assert_eq!(step.added_labels.len(), 1);
                    assert_eq!(step.conflict_notices.len(), 1);
                }
                other => panic!("unexpected forward result: {other:?}"),
            }

            let mut sel_rev = Vec::new();
            let _ = apply_mod_selection_input(&mut sel_rev, &b.to_string());
            let res_rev = apply_mod_selection_input(&mut sel_rev, &a.to_string());
            assert_eq!(sel_rev, vec![b], "reverse conflict failed for {b}<->{a}");
            match res_rev {
                ModSelectionCommand::Processed(step) => {
                    assert_eq!(step.added_labels, Vec::<&str>::new());
                    assert_eq!(step.conflict_notices.len(), 1);
                }
                other => panic!("unexpected reverse result: {other:?}"),
            }
        }
    }

    #[test]
    fn renders_multi_column_menu_with_bone_dagger_suffix_and_conflict_legend() {
        let lines = build_mod_menu_lines();
        let plain = lines
            .iter()
            .map(|l| strip_ansi(l))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.contains("7:骨の短剣モード [セット]"));
        assert!(plain.contains("3:マップ補強(山頂) <-> 21:観光モード(山頂)"));
        assert!(plain.contains("4:マップ補強(広漠) <-> 22:観光モード(広漠)"));
        assert!(plain.contains("12:暴風雪マスケット <-> 14:開幕マスケット"));
        assert!(plain.contains("21:観光モード(山頂) <-> 22:観光モード(広漠)"));
    }
}
