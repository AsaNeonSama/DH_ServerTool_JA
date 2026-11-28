use std::borrow::Cow;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::Mutex as AsyncMutex;

use crate::config::{
    ensure_server_working_dir, load_or_create_config, AppConfig, ConfigLoadOutcome, LoadedConfig,
    CONFIG_RELATIVE_PATH,
};
use crate::consts::gui::{
    APP_LICENSE_URL, CONTENT_MOD_ID_TOTEM_SPELLS, CONTENT_MOD_ID_UE_VOICE, GUI_APP_VERSION,
    MAX_COAL_BURN_RATE, MAX_CONSOLE_LOG_ENTRIES, MAX_DAYS_BEFORE_BLIZZARD, MAX_DAY_MINUTES,
    MAX_MAX_PLAYERS, MAX_MULTIPLIER_THREE, MAX_SERVER_LOG_ENTRIES, MAX_THRALLS, MIN_COAL_BURN_RATE,
    MIN_DAYS_BEFORE_BLIZZARD, MIN_DAY_MINUTES, MIN_MAX_PLAYERS, MIN_MULTIPLIER_QUARTER,
    MIN_THRALLS, PRACTICAL_MOD_MAX_NUMBER, REQ_BADGE_DLL, REQ_BADGE_PAK, REQ_DETAIL_TOTEM_SPELLS,
    REQ_DETAIL_UE_VOICE,
};
use crate::consts::server::{LEGACY_GADGET_DLL_FILENAME, MAP_OPTIONS};
use crate::format::format_local_time;
use crate::gui::console::{build_admin_console_catalog, default_console_draft};
use crate::languages;
use crate::mods::{
    find_content_option_by_number, find_option_by_number, get_mod_conflict_pairs, ModGroup,
    ModKind, ModMenuOption, PatchScript, ResolvedPatchScript, DEFAULT_PATCH_SCRIPTS,
    SELECTABLE_CONTENT_MOD_OPTIONS, SELECTABLE_MOD_OPTIONS,
};
use crate::server::supervisor::build_ue_map_argument;
use crate::structs::console::ConsoleLogRecord;
use crate::structs::gui::{
    ConflictPairDto, ConsoleLogEntryDto, DefaultModDto, GuiAppState, GuiSharedData, GuiSnapshot,
    LaunchServerRequestDto, MapOptionDto, MatchDraftDto, MatchSettingsDto, ModCatalogDto,
    ModOptionDto, PatchProgressCategory, PatchProgressItemDto, PatchProgressStatus,
    ServerLogEntryDto, ServerLogLevel, ServerMonitorStateDto, ServerPhase, UiSessionDraftDto,
};
use crate::structs::languages::{AppMsg, GuiMsg, GuiMsgCode};
use crate::structs::server::{MapOption, MatchConfig, ServerSettings};

impl From<ServerSettings> for MatchSettingsDto {
    fn from(s: ServerSettings) -> Self {
        Self {
            max_players: s.max_players,
            days_before_blizzard: s.days_before_blizzard,
            day_minutes: s.day_minutes,
            predator_damage: s.predator_damage,
            cold_intensity: s.cold_intensity,
            hunger_rate: s.hunger_rate,
            coal_burn_rate: s.coal_burn_rate,
            thralls: s.thralls,
        }
    }
}

impl From<MatchSettingsDto> for ServerSettings {
    fn from(dto: MatchSettingsDto) -> Self {
        Self {
            max_players: dto.max_players,
            days_before_blizzard: dto.days_before_blizzard,
            day_minutes: dto.day_minutes,
            predator_damage: dto.predator_damage,
            cold_intensity: dto.cold_intensity,
            hunger_rate: dto.hunger_rate,
            coal_burn_rate: dto.coal_burn_rate,
            thralls: dto.thralls,
        }
    }
}

#[must_use]
pub fn check_server_exe_status(shipping_executable: &str) -> (bool, bool) {
    let shipping_path: &Path = Path::new(shipping_executable);
    let exists: bool = shipping_path.is_file();
    let legacy_path: PathBuf = shipping_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(LEGACY_GADGET_DLL_FILENAME);
    let legacy_detected: bool = legacy_path.exists();
    (exists, legacy_detected)
}

/// Converts a picked absolute path into a clean `./relative/path` when located under `base_dir`,
/// preserving forward slashes for portable `launcher_config.json` storage.
#[must_use]
pub fn normalize_picked_exe_path(picked: &Path, base_dir: Option<&Path>) -> String {
    if let Some(base) = base_dir {
        if let Ok(rel) = picked.strip_prefix(base) {
            let rel_str: Cow<'_, str> = rel.to_string_lossy();
            let slash_rel: String = rel_str.replace('\\', "/");
            let trimmed: &str = slash_rel.trim_start_matches("./");
            return format!("./{trimmed}");
        }
    }
    picked.to_string_lossy().replace('\\', "/")
}

#[must_use]
pub fn client_requirement_metadata(mod_id: &str) -> (Option<&'static str>, Option<&'static str>) {
    match mod_id {
        CONTENT_MOD_ID_TOTEM_SPELLS => (Some(REQ_BADGE_PAK), Some(REQ_DETAIL_TOTEM_SPELLS)),
        CONTENT_MOD_ID_UE_VOICE => (Some(REQ_BADGE_DLL), Some(REQ_DETAIL_UE_VOICE)),
        _ => (Some(REQ_BADGE_PAK), None),
    }
}

fn map_mod_option_dto(opt: &ModMenuOption, is_client_required: bool) -> ModOptionDto {
    let kind: String = match opt.kind {
        ModKind::Single => "single".to_string(),
        ModKind::Mode => "mode".to_string(),
    };
    let (badge, detail): (Option<&'static str>, Option<&'static str>) = if is_client_required {
        client_requirement_metadata(opt.id)
    } else {
        (None, None)
    };
    let description: Option<String> =
        languages::mod_description(opt.label_code).map(str::to_string);
    let mut scripts: Vec<String> = opt
        .scripts
        .iter()
        .map(|s: &PatchScript| s.file_name.to_string())
        .collect();
    for dll in opt.dlls {
        scripts.push(dll.file_name().to_string());
    }
    let conflicts_with: Vec<String> = opt
        .conflicts_with
        .iter()
        .map(|c: &&str| (*c).to_string())
        .collect();

    ModOptionDto {
        id: opt.id.to_string(),
        number: opt.number,
        label: languages::mod_label(opt.label_code).to_string(),
        kind,
        requirement_badge: badge.map(str::to_string),
        requirement_detail: detail.map(str::to_string),
        description,
        scripts,
        conflicts_with,
    }
}

#[must_use]
pub fn build_mod_catalog_dtos() -> ModCatalogDto {
    let default_mods: Vec<DefaultModDto> = DEFAULT_PATCH_SCRIPTS
        .iter()
        .map(|patch: &PatchScript| DefaultModDto {
            id: patch.id.to_string(),
            file_name: patch.file_name.to_string(),
            label: languages::mod_label(patch.label_code).to_string(),
        })
        .collect();

    let client_required_mods: Vec<ModOptionDto> = SELECTABLE_CONTENT_MOD_OPTIONS
        .iter()
        .map(|opt: &ModMenuOption| map_mod_option_dto(opt, true))
        .collect();

    let mut practical_mods: Vec<ModOptionDto> = Vec::new();
    let mut fun_mods: Vec<ModOptionDto> = Vec::new();
    for opt in SELECTABLE_MOD_OPTIONS {
        let dto: ModOptionDto = map_mod_option_dto(opt, false);
        if opt.number <= PRACTICAL_MOD_MAX_NUMBER {
            practical_mods.push(dto);
        } else {
            fun_mods.push(dto);
        }
    }

    let conflict_pairs: Vec<ConflictPairDto> = get_mod_conflict_pairs()
        .into_iter()
        .map(|(a, b): (&ModMenuOption, &ModMenuOption)| ConflictPairDto {
            a_id: a.id.to_string(),
            a_number: a.number,
            a_label: languages::mod_label(a.label_code).to_string(),
            b_id: b.id.to_string(),
            b_number: b.number,
            b_label: languages::mod_label(b.label_code).to_string(),
        })
        .collect();

    ModCatalogDto {
        default_mods,
        client_required_mods,
        practical_mods,
        fun_mods,
        conflict_pairs,
    }
}

#[must_use]
pub fn build_map_option_dtos() -> Vec<MapOptionDto> {
    MAP_OPTIONS
        .iter()
        .map(|opt: &MapOption| MapOptionDto {
            key: opt.key.to_string(),
            id: opt.id.to_string(),
            label: languages::server_text(opt.label_code).to_string(),
        })
        .collect()
}

#[must_use]
pub fn idle_server_monitor_state(config: &AppConfig) -> ServerMonitorStateDto {
    ServerMonitorStateDto {
        phase: ServerPhase::Stopped,
        pid: None,
        active_port: None,
        next_port: None,
        switch_port_on_restart: config.switch_port_on_restart,
        server_ports: config.server_ports,
        udp_confirmed: false,
        round: 0,
        map_id: None,
        map_label: None,
        active_mod_count: 0,
        injected_count: 0,
        total_patch_count: 0,
        patch_progress: Vec::new(),
        countdown_seconds: None,
        current_patch_log_file: None,
        last_error: None,
    }
}

#[must_use]
pub fn build_patch_progress_list(patches: &[ResolvedPatchScript]) -> Vec<PatchProgressItemDto> {
    patches
        .iter()
        .enumerate()
        .map(|(idx, patch): (usize, &ResolvedPatchScript)| {
            let category: PatchProgressCategory = match patch.group {
                ModGroup::Default => PatchProgressCategory::Default,
                ModGroup::ContentMod => PatchProgressCategory::ClientRequired,
                ModGroup::Single | ModGroup::BoneDagger => PatchProgressCategory::ServerOnly,
            };
            PatchProgressItemDto {
                index: idx + 1,
                id: patch.id.clone(),
                file_name: patch.file_name.clone(),
                label: patch.label.clone(),
                category,
                status: PatchProgressStatus::Pending,
                error: None,
            }
        })
        .collect()
}

fn validate_range_u32(val: u32, min: u32, max: u32, name: &str) -> Result<(), String> {
    if val < min || val > max {
        let min_s: String = min.to_string();
        let max_s: String = max.to_string();
        return Err(languages::render_gui(&GuiMsg::SettingOutOfRange {
            name,
            min: &min_s,
            max: &max_s,
        }));
    }
    Ok(())
}

fn validate_range_f64(val: f64, min: f64, max: f64, name: &str) -> Result<(), String> {
    if !val.is_finite() || val < min || val > max {
        let min_s: String = min.to_string();
        let max_s: String = max.to_string();
        return Err(languages::render_gui(&GuiMsg::SettingOutOfRange {
            name,
            min: &min_s,
            max: &max_s,
        }));
    }
    Ok(())
}

/// Validates a GUI launch/update request and builds a strongly typed `MatchConfig`.
pub fn build_validated_match_config(req: &LaunchServerRequestDto) -> Result<MatchConfig, String> {
    let map_opt: MapOption = MAP_OPTIONS
        .iter()
        .copied()
        .find(|opt: &MapOption| opt.id == req.map_id)
        .ok_or_else(|| languages::gui_text(GuiMsgCode::InvalidMapSelection).to_string())?;

    let s: MatchSettingsDto = req.settings;
    validate_range_u32(s.max_players, MIN_MAX_PLAYERS, MAX_MAX_PLAYERS, "最大人数")?;
    validate_range_u32(
        s.days_before_blizzard,
        MIN_DAYS_BEFORE_BLIZZARD,
        MAX_DAYS_BEFORE_BLIZZARD,
        "暴風雪前の日数",
    )?;
    validate_range_u32(
        s.day_minutes,
        MIN_DAY_MINUTES,
        MAX_DAY_MINUTES,
        "一日の時間",
    )?;
    validate_range_u32(s.thralls, MIN_THRALLS, MAX_THRALLS, "傀儡人数")?;
    validate_range_f64(
        s.predator_damage,
        MIN_MULTIPLIER_QUARTER,
        MAX_MULTIPLIER_THREE,
        "野生動物ダメージ",
    )?;
    validate_range_f64(
        s.cold_intensity,
        MIN_MULTIPLIER_QUARTER,
        MAX_MULTIPLIER_THREE,
        "寒さ強度",
    )?;
    validate_range_f64(
        s.hunger_rate,
        MIN_MULTIPLIER_QUARTER,
        MAX_MULTIPLIER_THREE,
        "ハングリー強度",
    )?;
    validate_range_f64(
        s.coal_burn_rate,
        MIN_COAL_BURN_RATE,
        MAX_COAL_BURN_RATE,
        "石炭燃焼率",
    )?;

    let mut content_nums: Vec<u32> = Vec::new();
    let mut content_labels: Vec<String> = Vec::new();
    for &num in &req.selected_content_numbers {
        if let Some(opt) = find_content_option_by_number(num) {
            if !content_nums.contains(&num) {
                content_nums.push(num);
                content_labels.push(languages::mod_label(opt.label_code).to_string());
            }
        }
    }

    let mut option_nums: Vec<u32> = Vec::new();
    let mut option_labels: Vec<String> = Vec::new();
    for &num in &req.selected_option_numbers {
        if let Some(opt) = find_option_by_number(num) {
            if !option_nums.contains(&num) {
                option_nums.push(num);
                option_labels.push(languages::mod_label(opt.label_code).to_string());
            }
        }
    }

    let settings: ServerSettings = ServerSettings::from(s);
    let ue_map_argument: String = build_ue_map_argument(map_opt.id, &settings);

    Ok(MatchConfig {
        map_id: map_opt.id,
        map_label_code: map_opt.label_code,
        settings,
        selected_content_option_numbers: content_nums,
        selected_content_option_labels: content_labels,
        selected_option_numbers: option_nums,
        selected_option_labels: option_labels,
        ue_map_argument,
    })
}

impl GuiAppState {
    #[must_use]
    pub fn initialize() -> Self {
        dh_gadget_injector::sweep_default_stale_run_dirs();

        let default_cfg: AppConfig = AppConfig::default();
        let _: bool = ensure_server_working_dir(&default_cfg.shipping_executable);

        let loaded: LoadedConfig = load_or_create_config();
        let default_settings: MatchSettingsDto = MatchSettingsDto::from(ServerSettings::default());
        let initial_status: ServerMonitorStateDto = idle_server_monitor_state(&loaded.config);
        let ui_draft: UiSessionDraftDto = UiSessionDraftDto {
            active_tab: "console".to_string(),
            console_draft: default_console_draft(loaded.config.auto_broadcast_default),
            config_draft: loaded.config.clone(),
            auto_restart_watchdog: true,
            match_draft: MatchDraftDto {
                map_id: MAP_OPTIONS[0].id.to_string(),
                settings: default_settings,
                selected_content_numbers: Vec::new(),
                selected_option_numbers: Vec::new(),
            },
        };

        let shared: GuiSharedData = GuiSharedData {
            config: loaded.config,
            config_outcome: loaded.outcome,
            ui_draft,
            server_status: initial_status,
            console_logs: VecDeque::with_capacity(MAX_CONSOLE_LOG_ENTRIES),
            server_logs: VecDeque::with_capacity(MAX_SERVER_LOG_ENTRIES),
            next_log_id: 1,
        };

        Self {
            shared: Arc::new(Mutex::new(shared)),
            supervisor_handle: Arc::new(AsyncMutex::new(None)),
        }
    }

    pub fn lock_shared(&self) -> MutexGuard<'_, GuiSharedData> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[must_use]
    pub fn push_console_log(&self, record: ConsoleLogRecord) -> ConsoleLogEntryDto {
        let mut guard: MutexGuard<'_, GuiSharedData> = self.lock_shared();
        let id: u64 = guard.next_log_id;
        guard.next_log_id = guard.next_log_id.saturating_add(1);
        let entry: ConsoleLogEntryDto = ConsoleLogEntryDto {
            id,
            time: record.time,
            category: record.category,
            tone: record.tone,
            detail: record.detail,
        };
        if guard.console_logs.len() >= MAX_CONSOLE_LOG_ENTRIES {
            let _: Option<ConsoleLogEntryDto> = guard.console_logs.pop_front();
        }
        guard.console_logs.push_back(entry.clone());
        entry
    }

    #[must_use]
    pub fn push_server_log(&self, level: ServerLogLevel, message: String) -> ServerLogEntryDto {
        let mut guard: MutexGuard<'_, GuiSharedData> = self.lock_shared();
        let id: u64 = guard.next_log_id;
        guard.next_log_id = guard.next_log_id.saturating_add(1);
        let entry: ServerLogEntryDto = ServerLogEntryDto {
            id,
            time: format_local_time(),
            level,
            message,
        };
        if guard.server_logs.len() >= MAX_SERVER_LOG_ENTRIES {
            let _: Option<ServerLogEntryDto> = guard.server_logs.pop_front();
        }
        guard.server_logs.push_back(entry.clone());
        entry
    }

    #[must_use]
    pub fn build_snapshot(&self) -> GuiSnapshot {
        let guard: MutexGuard<'_, GuiSharedData> = self.lock_shared();
        let (server_exe_exists, legacy_frida_dll_detected): (bool, bool) =
            check_server_exe_status(&guard.config.shipping_executable);

        let (config_outcome, config_notice): (String, Option<String>) = match &guard.config_outcome
        {
            ConfigLoadOutcome::Loaded => ("loaded".to_string(), None),
            ConfigLoadOutcome::CreatedDefault => (
                "created_default".to_string(),
                Some(languages::render_app(&AppMsg::ConfigCreated {
                    relative_path: CONFIG_RELATIVE_PATH,
                })),
            ),
            ConfigLoadOutcome::CreateFailed => ("create_failed".to_string(), None),
            ConfigLoadOutcome::LoadFailed(err) => (
                "load_failed".to_string(),
                Some(languages::render_app(&AppMsg::ConfigLoadFailed {
                    error_text: err,
                })),
            ),
        };

        let maps: Vec<MapOptionDto> = build_map_option_dtos();
        let catalog: ModCatalogDto = build_mod_catalog_dtos();

        GuiSnapshot {
            app_version: GUI_APP_VERSION.to_string(),
            license_url: APP_LICENSE_URL.to_string(),
            config: guard.config.clone(),
            config_outcome,
            config_notice,
            server_exe_exists,
            legacy_frida_dll_detected,
            auto_restart_watchdog: guard.ui_draft.auto_restart_watchdog,
            console_catalog: build_admin_console_catalog(),
            maps,
            default_match_settings: MatchSettingsDto::from(ServerSettings::default()),
            default_mods: catalog.default_mods,
            client_required_mods: catalog.client_required_mods,
            practical_mods: catalog.practical_mods,
            fun_mods: catalog.fun_mods,
            conflict_pairs: catalog.conflict_pairs,
            ui_draft: guard.ui_draft.clone(),
            server_status: guard.server_status.clone(),
            console_logs: guard.console_logs.iter().cloned().collect(),
            server_logs: guard.server_logs.iter().cloned().collect(),
        }
    }
}
