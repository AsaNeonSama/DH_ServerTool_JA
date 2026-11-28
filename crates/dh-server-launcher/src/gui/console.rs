use rand::RngExt;

use crate::console::dispatch;
use crate::consts::console::{status_code, PLAYER_NAME_LIST};
use crate::consts::gui::{
    CANNIBAL_RANDOM_KEY, CANNIBAL_VARIANTS, CONSOLE_BLIZZARD_INTENSITIES, CONSOLE_COMMAND_TYPES,
    CONSOLE_ITEM_COUNTS, CONSOLE_ITEM_PATHS, CONSOLE_SLOMO_OPTIONS, CONSOLE_SPELL_CHARGES,
    CONSOLE_STATUS_SPECS, CONSOLE_STORAGE_OPTIONS, CONSOLE_SUMMON_KEYS,
};
use crate::languages;
use crate::structs::console::{
    ConsoleActionPayload, ConsoleBridge, ConsoleDispatchContext, ConsoleLogSink,
};
use crate::structs::gui::{
    AdminConsoleActionRequestDto, AdminConsoleActionResultDto, AdminConsoleCatalogDto,
    ConsoleChoiceDto, ConsoleDraftDto, ConsoleStatusSpecDto,
};

#[must_use]
pub fn default_console_draft(auto_broadcast: bool) -> ConsoleDraftDto {
    ConsoleDraftDto {
        operation: "console_command".to_string(),
        target_mode: "role".to_string(),
        target_role: PLAYER_NAME_LIST[0].to_string(),
        custom_target: String::new(),
        auto_broadcast,
        console_order: "summon".to_string(),
        summon_target: CONSOLE_SUMMON_KEYS[0].to_string(),
        is_thrall: "1".to_string(),
        spell_charge: CONSOLE_SPELL_CHARGES[0].0.to_string(),
        blizzard_intensity: CONSOLE_BLIZZARD_INTENSITIES[0].0.to_string(),
        slomo: CONSOLE_SLOMO_OPTIONS[0].0.to_string(),
        god: "1".to_string(),
        end_match: "1".to_string(),
        raw_command: String::new(),
        status_prop: CONSOLE_STATUS_SPECS[0].0.to_string(),
        status_toggle: "1".to_string(),
        status_number: "100".to_string(),
        item_filter: String::new(),
        item_path: CONSOLE_ITEM_PATHS[0].to_string(),
        item_count: CONSOLE_ITEM_COUNTS[0].to_string(),
        storage_limit: CONSOLE_STORAGE_OPTIONS[0].0.to_string(),
        chat_broadcast: true,
        chat_message: String::new(),
    }
}

#[must_use]
pub fn build_admin_console_catalog() -> AdminConsoleCatalogDto {
    let roles: Vec<ConsoleChoiceDto> = PLAYER_NAME_LIST
        .iter()
        .map(|&role: &&str| ConsoleChoiceDto {
            value: role.to_string(),
            label: languages::player_role_name(role)
                .unwrap_or(role)
                .to_string(),
        })
        .collect();

    let command_types: Vec<ConsoleChoiceDto> = CONSOLE_COMMAND_TYPES
        .iter()
        .map(|&(value, label): &(&str, &str)| ConsoleChoiceDto {
            value: value.to_string(),
            label: label.to_string(),
        })
        .collect();

    let summons: Vec<ConsoleChoiceDto> = CONSOLE_SUMMON_KEYS
        .iter()
        .map(|&key: &&str| {
            let label: &str = if key == CANNIBAL_RANDOM_KEY {
                "人喰い人種 (ランダム)"
            } else {
                languages::actor_display_name(key).unwrap_or(key)
            };
            ConsoleChoiceDto {
                value: key.to_string(),
                label: label.to_string(),
            }
        })
        .collect();

    let spell_charges: Vec<ConsoleChoiceDto> = CONSOLE_SPELL_CHARGES
        .iter()
        .map(|&(value, label): &(&str, &str)| ConsoleChoiceDto {
            value: value.to_string(),
            label: label.to_string(),
        })
        .collect();

    let blizzard_intensities: Vec<ConsoleChoiceDto> = CONSOLE_BLIZZARD_INTENSITIES
        .iter()
        .map(|&(value, label): &(&str, &str)| ConsoleChoiceDto {
            value: value.to_string(),
            label: label.to_string(),
        })
        .collect();

    let slomos: Vec<ConsoleChoiceDto> = CONSOLE_SLOMO_OPTIONS
        .iter()
        .map(|&(value, label): &(&str, &str)| ConsoleChoiceDto {
            value: value.to_string(),
            label: label.to_string(),
        })
        .collect();

    let statuses: Vec<ConsoleStatusSpecDto> = CONSOLE_STATUS_SPECS
        .iter()
        .map(
            |&(value, label, is_toggle): &(&str, &str, bool)| ConsoleStatusSpecDto {
                value: value.to_string(),
                label: label.to_string(),
                is_toggle,
            },
        )
        .collect();

    let items: Vec<ConsoleChoiceDto> = CONSOLE_ITEM_PATHS
        .iter()
        .map(|&path: &&str| ConsoleChoiceDto {
            value: path.to_string(),
            label: languages::actor_display_name(path)
                .unwrap_or(path)
                .to_string(),
        })
        .collect();

    let storages: Vec<ConsoleChoiceDto> = CONSOLE_STORAGE_OPTIONS
        .iter()
        .map(|&(value, label): &(&str, &str)| ConsoleChoiceDto {
            value: value.to_string(),
            label: label.to_string(),
        })
        .collect();

    AdminConsoleCatalogDto {
        roles,
        command_types,
        summons,
        spell_charges,
        blizzard_intensities,
        slomos,
        statuses,
        items,
        item_counts: CONSOLE_ITEM_COUNTS.to_vec(),
        storages,
    }
}

fn resolve_command_random_tokens(command: &str) -> String {
    if !command.contains(CANNIBAL_RANDOM_KEY) {
        return command.to_string();
    }
    let idx: usize = rand::rng().random_range(0..CANNIBAL_VARIANTS.len());
    let chosen: &str = CANNIBAL_VARIANTS[idx];
    command.replace(CANNIBAL_RANDOM_KEY, chosen)
}

/// Dispatches an in-app Admin Console action directly over the local loopback `ConsoleBridge`
/// (`127.0.0.1:<consolePatchPort>`) to `コンソール連携.js` with zero HTTP/network exposure.
pub async fn dispatch_admin_console_action(
    bridge: &ConsoleBridge,
    log_sink: &ConsoleLogSink,
    req: &AdminConsoleActionRequestDto,
) -> AdminConsoleActionResultDto {
    let payload: ConsoleActionPayload = match req {
        AdminConsoleActionRequestDto::ConsoleCommand {
            player_name,
            command,
            auto_broadcast,
        } => {
            let resolved_cmd: String = resolve_command_random_tokens(command.trim());
            let ctx: ConsoleDispatchContext<'_> = ConsoleDispatchContext {
                bridge,
                auto_broadcast: *auto_broadcast,
                log_sink,
            };
            dispatch::handle_console_command(&ctx, player_name, &resolved_cmd).await
        }
        AdminConsoleActionRequestDto::SetHuman {
            player_name,
            prop_name,
            value,
            auto_broadcast,
        } => {
            let ctx: ConsoleDispatchContext<'_> = ConsoleDispatchContext {
                bridge,
                auto_broadcast: *auto_broadcast,
                log_sink,
            };
            dispatch::handle_set_human(&ctx, player_name, prop_name, value).await
        }
        AdminConsoleActionRequestDto::AddItem {
            player_name,
            item_name,
            count,
            auto_broadcast,
        } => {
            let ctx: ConsoleDispatchContext<'_> = ConsoleDispatchContext {
                bridge,
                auto_broadcast: *auto_broadcast,
                log_sink,
            };
            dispatch::handle_add_item(&ctx, player_name, item_name, count).await
        }
        AdminConsoleActionRequestDto::SetStorage {
            player_name,
            limit,
            auto_broadcast,
        } => {
            let ctx: ConsoleDispatchContext<'_> = ConsoleDispatchContext {
                bridge,
                auto_broadcast: *auto_broadcast,
                log_sink,
            };
            dispatch::handle_set_storage(&ctx, player_name, limit).await
        }
        AdminConsoleActionRequestDto::SendThrallMsg {
            player_name,
            msg,
            broadcast,
        } => {
            let ctx: ConsoleDispatchContext<'_> = ConsoleDispatchContext {
                bridge,
                auto_broadcast: false,
                log_sink,
            };
            if *broadcast {
                dispatch::handle_broadcast(&ctx, msg).await
            } else {
                dispatch::handle_send_thrall_msg(&ctx, player_name, msg).await
            }
        }
        AdminConsoleActionRequestDto::KickPlayer {
            player_name,
            auto_broadcast,
        } => {
            let ctx: ConsoleDispatchContext<'_> = ConsoleDispatchContext {
                bridge,
                auto_broadcast: *auto_broadcast,
                log_sink,
            };
            dispatch::handle_kick_player(&ctx, player_name).await
        }
    };

    AdminConsoleActionResultDto {
        ok: payload.status == status_code::SUCCESS,
        status: payload.status,
        code: payload.code,
        message: languages::api_text(payload.code).to_string(),
    }
}
