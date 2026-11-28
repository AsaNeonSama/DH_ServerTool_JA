use serde::{Deserialize, Serialize};

/// Supported UI / log display locales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    Ja,
}

/// Wire status code returned by in-app Admin Console actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiCode {
    #[serde(rename = "API_SUCCESS")]
    Success,
    #[serde(rename = "API_NETWORK_PARAM_ERROR")]
    NetworkParamError,
    #[serde(rename = "API_SERVER_EXECUTION_ERROR")]
    ServerExecutionError,
    #[serde(rename = "API_INVALID_VALUE_ERROR")]
    InvalidValueError,
    #[serde(rename = "API_HIGH_VALUE_WARNING")]
    HighValueWarning,
    #[serde(rename = "API_BROADCAST_PARTIAL_ERROR")]
    BroadcastPartialError,
}

/// Category code for in-app Admin Console log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogCategoryCode {
    Command,
    Status,
    Item,
    Storage,
    Notice,
    Broadcast,
    Kick,
}

/// Result badge code appended to Admin Console action logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultBadgeCode {
    Sent,
    PartialFailure,
    SendFailed,
    Invalid,
}

/// Static detail reason / fragment code used when rendering Admin Console logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLogDetailCode {
    InvalidParams,
    InvalidNumberReason,
    InvalidCountReason,
    InvalidValueReason,
    BoneDaggerAddedSuffix,
    StorageCapacityLabel,
    BroadcastAllTarget,
    KickVerb,
    EndMatchExplorersWin,
    EndMatchThrallsWin,
}

/// Structured Admin Console log event rendered cleanly without ANSI escape sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLogEvent<'a> {
    InvalidParams {
        category: LogCategoryCode,
    },
    CommandResult {
        role_key: &'a str,
        command: &'a str,
        added_bone_dagger: bool,
        status: u16,
        err_detail: Option<&'a str>,
    },
    StatusInvalidValue {
        role_key: &'a str,
        prop_name: &'a str,
        raw_value: &'a str,
    },
    StatusResult {
        role_key: &'a str,
        prop_name: &'a str,
        value_text: &'a str,
        status: u16,
        err_detail: Option<&'a str>,
    },
    ItemInvalidCount {
        role_key: &'a str,
        item_key: &'a str,
        raw_count: &'a str,
    },
    ItemResult {
        role_key: &'a str,
        item_key: &'a str,
        count: i64,
        status: u16,
        err_detail: Option<&'a str>,
    },
    StorageInvalidValue {
        role_key: &'a str,
        raw_limit: &'a str,
    },
    StorageResult {
        role_key: &'a str,
        limit: i64,
        status: u16,
        err_detail: Option<&'a str>,
    },
    NoticeResult {
        role_key: &'a str,
        message: &'a str,
        status: u16,
        err_detail: Option<&'a str>,
    },
    BroadcastResult {
        message: &'a str,
        status: u16,
        err_detail: Option<&'a str>,
    },
    KickResult {
        role_key: &'a str,
        status: u16,
        err_detail: Option<&'a str>,
    },
}

/// Structured in-game broadcast notice event rendered via `crate::languages`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BroadcastEvent<'a> {
    Summon {
        role_key: &'a str,
        target_key: &'a str,
    },
    UnsetThrall {
        role_key: &'a str,
    },
    SetThrall {
        role_key: &'a str,
    },
    SpellCharge {
        role_key: &'a str,
    },
    StopBlizzard {
        role_key: &'a str,
    },
    StartBlizzard {
        role_key: &'a str,
    },
    ResetTimeScale {
        role_key: &'a str,
    },
    AccelerateTimeScale {
        role_key: &'a str,
    },
    DisableGodMode {
        role_key: &'a str,
    },
    EnableGodMode {
        role_key: &'a str,
    },
    ModifiedStats {
        role_key: &'a str,
    },
    AcquiredItem {
        role_key: &'a str,
        item_key: &'a str,
        count: i64,
    },
    StorageLimitChanged {
        role_key: &'a str,
        limit: i64,
    },
    Kicked {
        role_key: &'a str,
    },
}

/// Parameterized application-level message events.
#[derive(Debug, Clone, Copy)]
pub enum AppMsg<'a> {
    ConfigCreated { relative_path: &'a str },
    ConfigLoadFailed { error_text: &'a str },
}

/// Display label code for every default and selectable MOD / script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModLabelCode {
    DefaultConsole,
    DefaultSpellChargeFix,
    DefaultDeathOfflineAnnouncement,
    DefaultNetworkFix,
    DefaultCannibalClimb,
    ContentTotemSpells,
    ContentUeVoice,
    RandRole,
    SpellCastStart,
    ResourcesSpawnerDeparture,
    ResourcesSpawnerExpanse,
    SpellCustom,
    MadmateNormalNolimit,
    BoneDaggerMode,
    BoneDaggerScriptMain,
    BoneDaggerScriptDropTable,
    BoneDaggerScriptInventory,
    QuestSystem,
    AddStorage,
    WhetstoneAnyItem,
    ArrowFullRecovery,
    BlizzardArmoryOpen,
    GunAndBow,
    StartArmoryOpen,
    VomitModify,
    NorthGhost,
    Grenade,
    MoonJump,
    SpellNoCd,
    NoDeath,
    SightseeingDeparture,
    SightseeingExpanse,
}

/// Static server supervisor & map label message codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerMsgCode {
    MapLabel1,
    MapLabel2,
    MapLabel3,
}

/// Parameterized server supervisor log events.
#[derive(Debug, Clone, Copy)]
pub enum ServerMsg<'a> {
    SpawningSuspended {
        executable: &'a str,
        port: u16,
    },
    PatchInitError {
        file_name: &'a str,
        error_text: &'a str,
    },
    InjectedPatchDone {
        index: usize,
        total: usize,
        label: &'a str,
    },
    AllPatchesInjectedResumed {
        count: usize,
    },
    PatchLogHeader {
        timestamp: &'a str,
        port: u16,
        patch_names: &'a str,
    },
    RoundLaunchHeader {
        round: u32,
        port: u16,
    },
    StartupInjectError {
        error_text: &'a str,
    },
    ContentModAssetPrepareError {
        error_text: &'a str,
    },
    StartupRetryAfter {
        delay_text: &'a str,
        switch_port_on_restart: bool,
    },
    MonitoringServer {
        interval_text: &'a str,
        switch_port_on_restart: bool,
    },
    UdpReadyConfirmed {
        pid: u32,
        port: u16,
    },
    UdpStoppedRestarting {
        port: u16,
    },
}

/// Static desktop GUI message codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiMsgCode {
    ServerAlreadyRunning,
    ServerNotRunning,
    InvalidMapSelection,
    ServerStoppedCleanly,
    WatchdogOffStopped,
    ImmediateRestartRequested,
    CountdownCancelledForSetup,
    NextRoundPlanUpdated,
    LogDirResolveFailed,
}

/// Parameterized desktop GUI messages.
#[derive(Debug, Clone, Copy)]
pub enum GuiMsg<'a> {
    SettingOutOfRange {
        name: &'a str,
        min: &'a str,
        max: &'a str,
    },
    ServerExeNotFound {
        path: &'a str,
    },
    OpenFolderFailed {
        error_text: &'a str,
    },
}
