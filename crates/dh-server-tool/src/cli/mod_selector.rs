use tokio_util::sync::CancellationToken;

use crate::colors;
pub use crate::consts::cli::{MOD_MENU_COLS, MOD_MENU_COL_WIDTH};
use crate::languages;
use crate::mods::{
    find_conflicting_selected_options, find_option_by_number, get_mod_conflict_pairs, ModKind,
    ModMenuOption, PatchScript, DEFAULT_PATCH_SCRIPTS, SELECTABLE_MOD_OPTIONS,
};
pub use crate::structs::cli::{ModSelectionCommand, ModSelectionStepResult};
use crate::structs::languages::{CliMsg, CliMsgCode};
use crate::terminal::{pad_display_width, PromptError, Terminal};

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
                languages::cli_text(CliMsgCode::ModModeSuffix)
            } else {
                ""
            };
            let opt_label: &str = languages::mod_label(opt.label_code);
            let cell: String = format!("{}:{opt_label}{suffix}", opt.number);
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
                    format!(
                        "{}:{} <-> {}:{}",
                        a.number,
                        languages::mod_label(a.label_code),
                        b.number,
                        languages::mod_label(b.label_code)
                    )
                },
            )
            .collect::<Vec<String>>()
            .join(" / ");
        lines.push(format!(
            "{} * {} {pair_text}",
            colors::LIGHT_BLACK,
            languages::cli_text(CliMsgCode::ModConflictLegendPrefix),
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
                |item: &ModMenuOption| languages::mod_label(item.label_code).to_string(),
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

        let opt_label: &'static str = languages::mod_label(opt.label_code);

        if selected_numbers.contains(&opt.number) {
            if !duplicate_labels.contains(&opt_label) {
                duplicate_labels.push(opt_label);
            }
            continue;
        }

        let conflicts: Vec<&'static ModMenuOption> =
            find_conflicting_selected_options(opt, selected_numbers);
        if !conflicts.is_empty() {
            let conflict_targets: String = conflicts
                .iter()
                .map(|item: &&ModMenuOption| {
                    format!("{}:{}", item.number, languages::mod_label(item.label_code))
                })
                .collect::<Vec<String>>()
                .join(", ");
            let option_text: String = format!("{}:{opt_label}", opt.number);
            let notice: String = languages::render_cli(&CliMsg::ModConflictDetail {
                option_text: &option_text,
                targets: &conflict_targets,
            });
            if !conflict_notices.contains(&notice) {
                conflict_notices.push(notice);
            }
            continue;
        }

        selected_numbers.push(opt.number);
        added_labels.push(opt_label);
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
        .map(|patch: &PatchScript| languages::mod_label(patch.label_code))
        .collect::<Vec<&str>>()
        .join(", ");

    terminal.println(&format!(
        "{} * {} {}{default_labels}",
        colors::LIGHT_CYAN,
        languages::cli_text(CliMsgCode::DefaultModsLabel),
        colors::LIGHT_GREEN,
    ));
    terminal.println(&format!(
        "{} {} {}Y/y {}{} {}C/c {}{}",
        colors::LIGHT_YELLOW,
        languages::cli_text(CliMsgCode::ModSelectionGuideBeforeY),
        colors::LIGHT_GREEN,
        colors::LIGHT_YELLOW,
        languages::cli_text(CliMsgCode::ModSelectionGuideBetweenYAndC),
        colors::LIGHT_GREEN,
        colors::LIGHT_YELLOW,
        languages::cli_text(CliMsgCode::ModSelectionGuideAfterC),
    ));
    terminal.println_empty();

    for line in build_mod_menu_lines() {
        terminal.println(&line);
    }

    let prompt: String = format!(
        "{} * {} ",
        colors::LIGHT_CYAN,
        languages::cli_text(CliMsgCode::ModChoicePrompt)
    );
    let mut selected_numbers: Vec<u32> = Vec::new();

    loop {
        let raw_choice: String = terminal.ask(&prompt, cancel).await?;
        match apply_mod_selection_input(&mut selected_numbers, &raw_choice) {
            ModSelectionCommand::Confirm => break,
            ModSelectionCommand::Cleared => {
                terminal.println(&format!(
                    "{} * {}",
                    colors::LIGHT_YELLOW,
                    languages::cli_text(CliMsgCode::ModSelectionCleared)
                ));
            }
            ModSelectionCommand::EmptyInvalid => {
                terminal.println(&format!(
                    "{} * {}",
                    colors::RED,
                    languages::cli_text(CliMsgCode::InvalidChoiceShort)
                ));
            }
            ModSelectionCommand::Processed(step) => {
                if !step.added_labels.is_empty() {
                    let current_names: String = format_selected_labels(&selected_numbers);
                    terminal.println(&format!(
                        "{} * {} {} {}{}",
                        colors::LIGHT_GREEN,
                        languages::cli_text(CliMsgCode::ModAddedPrefix),
                        step.added_labels.join(", "),
                        colors::LIGHT_BLACK,
                        languages::render_cli(&CliMsg::ModCurrentSelection {
                            names: &current_names,
                        }),
                    ));
                }
                if !step.duplicate_labels.is_empty() {
                    let current_names: String = format_selected_labels(&selected_numbers);
                    terminal.println(&format!(
                        "{} * {} {} {}{}",
                        colors::LIGHT_YELLOW,
                        languages::cli_text(CliMsgCode::ModDuplicatePrefix),
                        step.duplicate_labels.join(", "),
                        colors::LIGHT_BLACK,
                        languages::render_cli(&CliMsg::ModCurrentSelection {
                            names: &current_names,
                        }),
                    ));
                }
                if !step.conflict_notices.is_empty() {
                    terminal.println(&format!(
                        "{} * {} {}",
                        colors::RED,
                        languages::cli_text(CliMsgCode::ModConflictSkippedPrefix),
                        step.conflict_notices.join(" / "),
                    ));
                }
                if !step.invalid_tokens.is_empty() {
                    let invalid_joined: String = step.invalid_tokens.join(", ");
                    terminal.println(&format!(
                        "{} * {}",
                        colors::RED,
                        languages::render_cli(&CliMsg::InvalidChoiceList {
                            items: &invalid_joined,
                        }),
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
                |item: &ModMenuOption| languages::mod_label(item.label_code).to_string(),
            )
        })
        .collect();

    Ok((selected_numbers, selected_labels))
}
