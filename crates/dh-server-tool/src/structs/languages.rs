use serde::{Deserialize, Serialize};

/// Supported UI / log display locales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    Ja,
}

/// Wire status code returned by encrypted Web Console API endpoints.
/// The frontend resolves localized user-facing text from `assets/languages/*.json` using this code.
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

/// Category code for Web Console (Tab 1) log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogCategoryCode {
    Auth,
    Error,
    Command,
    Status,
    Item,
    Storage,
    Notice,
    Broadcast,
    Kick,
    Startup,
    Shutdown,
    Copy,
}

/// Result badge code appended to Web Console (Tab 1) action logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultBadgeCode {
    Sent,
    PartialFailure,
    SendFailed,
    Invalid,
}

/// Static detail reason / fragment code used when rendering Web Console logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebLogDetailCode {
    InvalidParams,
    InvalidNumberReason,
    InvalidCountReason,
    InvalidValueReason,
    BoneDaggerAddedSuffix,
    StorageCapacityLabel,
    BroadcastAllTarget,
    KickVerb,
    ShutdownDetail,
    EndMatchExplorersWin,
    EndMatchThrallsWin,
}

/// Structured Web Console (Tab 1) log event.
/// Business logic emits only codes and raw values; `web_console::logger` resolves localized
/// text from `crate::languages` right before rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebLogEvent<'a> {
    AuthUnauthenticatedRejected {
        client_ip: &'a str,
        pathname: &'a str,
    },
    AuthInvalidRequestBody {
        client_ip: &'a str,
        pathname: &'a str,
    },
    AuthDecryptFailed {
        client_ip: &'a str,
        pathname: &'a str,
    },
    AuthInvalidJsonParams {
        client_ip: &'a str,
        pathname: &'a str,
    },
    AuthLoginFailedInvalidBody {
        client_ip: &'a str,
    },
    AuthLoginFailedPskMismatch {
        client_ip: &'a str,
    },
    AuthLoginFailedChallengeInvalid {
        client_ip: &'a str,
    },
    AuthLoginSucceeded {
        client_ip: &'a str,
    },
    Startup {
        web_port: u16,
        patch_port: u16,
        auto_broadcast: bool,
    },
    Shutdown,
    UrlCopied {
        is_public: bool,
        success: bool,
    },
    InvalidParams {
        category: LogCategoryCode,
        client_ip: &'a str,
    },
    CommandResult {
        client_ip: &'a str,
        role_key: &'a str,
        command: &'a str,
        added_bone_dagger: bool,
        status: u16,
        err_detail: Option<&'a str>,
    },
    StatusInvalidValue {
        client_ip: &'a str,
        role_key: &'a str,
        prop_name: &'a str,
        raw_value: &'a str,
    },
    StatusResult {
        client_ip: &'a str,
        role_key: &'a str,
        prop_name: &'a str,
        value_text: &'a str,
        status: u16,
        err_detail: Option<&'a str>,
    },
    ItemInvalidCount {
        client_ip: &'a str,
        role_key: &'a str,
        item_key: &'a str,
        raw_count: &'a str,
    },
    ItemResult {
        client_ip: &'a str,
        role_key: &'a str,
        item_key: &'a str,
        count: i64,
        status: u16,
        err_detail: Option<&'a str>,
    },
    StorageInvalidValue {
        client_ip: &'a str,
        role_key: &'a str,
        raw_limit: &'a str,
    },
    StorageResult {
        client_ip: &'a str,
        role_key: &'a str,
        limit: i64,
        status: u16,
        err_detail: Option<&'a str>,
    },
    NoticeResult {
        client_ip: &'a str,
        role_key: &'a str,
        message: &'a str,
        status: u16,
        err_detail: Option<&'a str>,
    },
    BroadcastResult {
        client_ip: &'a str,
        message: &'a str,
        status: u16,
        err_detail: Option<&'a str>,
    },
    KickResult {
        client_ip: &'a str,
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

/// Parameters for rendering the startup `config.json` summary line.
#[derive(Debug, Clone, Copy)]
pub struct ConfigSummaryParams<'a> {
    pub server_port_1: u16,
    pub server_port_2: u16,
    pub switch_port_on_restart: bool,
    pub web_console_port: u16,
    pub console_patch_port: u16,
    pub check_interval_text: &'a str,
    pub restart_wait_seconds: u64,
}

/// Static application-level message codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMsgCode {
    AuthorLabel,
    AuthorName,
    BrandBuild,
    BrandWith,
    BrandAntigravity,
    MissingServerDirError,
    PublicIpLabel,
    ShutdownSignalReceived,
    SummaryHeader,
    SummaryFooter,
    SummaryMapLabel,
    SummaryServerPortLabel,
    SummaryServerPortSwitchOnSuffix,
    SummaryServerPortFixedSuffix,
    SummaryWebConsoleLabel,
    SummaryDefaultModsLabel,
    SummaryContentModsLabel,
    SummarySelectedModsLabel,
    NoneLabel,
    ExitedCleanly,
    UnexpectedErrorHeader,
}

/// Parameterized application-level message events.
#[derive(Debug, Clone, Copy)]
pub enum AppMsg<'a> {
    PublicIpFetchFailed {
        error_text: &'a str,
    },
    MissingShippingExecutable {
        path: &'a str,
    },
    LegacyFridaDllDetected {
        file_name: &'a str,
        full_path: &'a str,
    },
    LegacyFridaDllAdvice {
        process_name: &'a str,
    },
    ConfigSummary(ConfigSummaryParams<'a>),
    ConfigCreated {
        relative_path: &'a str,
    },
    ConfigLoadFailed {
        error_text: &'a str,
    },
    WebConsoleStartFailed {
        port: u16,
        error_text: &'a str,
    },
    SummaryInternalPatchPort {
        port: u16,
    },
}

/// Validation error codes returned by CLI port validation helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliErrorCode {
    PortOutOfRange { min: u16, max: u16 },
    WebPortConflictsWithPatch { patch_port: u16 },
    ServerPortsDuplicate,
    ServerPortConflictsWithWeb { web_port: u16, patch_port: u16 },
}

/// Static CLI prompt & label codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliMsgCode {
    MapLabel1,
    MapLabel2,
    MapLabel3,
    WebConsoleSectionHeader,
    ServerSetupSectionHeader,
    ReconfigureSectionHeader,
    WebPortPrompt,
    AutoBroadcastPromptPrefix,
    AutoBroadcastChoices,
    SwitchPortOnRestartPromptPrefix,
    SwitchPortOnRestartChoices,
    ServerPortPromptPrefix,
    ServerPortsSelectedLabel,
    ServerPortSwitchBadge,
    ServerPortFixedBadge,
    MapPromptPrefix,
    MapPromptChoices,
    ServerConfigPromptPrefix,
    ServerConfigChoices,
    SettingMaxPlayers,
    SettingDaysBeforeBlizzard,
    SettingDayMinutes,
    SettingPredatorDamage,
    SettingColdIntensity,
    SettingHungerRate,
    SettingCoalBurnRate,
    SettingThralls,
    InvalidChoiceRetry,
    InvalidChoiceShort,
    ContentModsLabel,
    ContentModsGuideBeforeY,
    ContentModChoicePrompt,
    ContentModPakSuffix,
    DefaultModsLabel,
    ServerOnlyModsLabel,
    ModSelectionGuideBeforeY,
    ModSelectionGuideBetweenYAndC,
    ModSelectionGuideAfterC,
    ModChoicePrompt,
    ModModeSuffix,
    ModConflictLegendPrefix,
    ModSelectionCleared,
    ModAddedPrefix,
    ModDuplicatePrefix,
    ModConflictSkippedPrefix,
}

/// Parameterized CLI messages.
#[derive(Debug, Clone, Copy)]
pub enum CliMsg<'a> {
    InvalidChoiceList {
        items: &'a str,
    },
    ModConflictDetail {
        option_text: &'a str,
        targets: &'a str,
    },
    ModCurrentSelection {
        names: &'a str,
    },
    ReconfigurePreservedNotice {
        port_1: u16,
        port_2: u16,
        switch_port_on_restart: bool,
    },
    ReconfigureInterruptedNotice {
        error_text: &'a str,
    },
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

/// Static server supervisor message codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerMsgCode {
    SpawnedProcessPrefix,
    RoundSummaryRetryTitle,
    RoundSummaryUpdatedTitle,
    RoundSummaryContinuedTitle,
    RoundSummaryFooter,
    RoundSummaryFixedPortSuffix,
    RoundMapLabel,
    RoundPortLabel,
    ExitHint,
    ServerExitDetectedPrefix,
    ServerExitFixedPortSuffix,
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
    RetryNotice {
        error_text: &'a str,
        switch_port_on_restart: bool,
    },
    RoundSummaryCurrentPort {
        port: u16,
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

/// Static terminal / TUI message codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMsgCode {
    PressEnterToExit,
    PostMatchCountdownPrefix,
    PostMatchCountdownMid,
    PostMatchCountdownReconfigure,
    PostMatchCountdownRestart,
    Tab2Title,
    TabSwitchHint,
    WebSettingsHeader,
    WebTcpPortLabel,
    WebAutoBroadcastLabel,
    ToggleOn,
    ToggleOff,
    WebStartedHeader,
    WebStartedFooter,
    PublicUrlLabel,
    LocalUrlLabel,
    PskNoticeLead,
    PskNoticeBody,
    PskNoticeWarn,
    CopyPublicBtnLabel,
    CopyLocalBtnLabel,
    PublicUrlShortLabel,
    LocalUrlShortLabel,
    CopyFailureFeedback,
    CopyGuideHint,
    ServerPendingInputHint,
    SwitchToServerHint,
    WebLogBarControlsHint,
    WebLogEmptyPlaceholder,
}

/// Parameterized terminal / TUI message events.
#[derive(Debug, Clone, Copy)]
pub enum TerminalMsg<'a> {
    Tab1Title {
        port_suffix: &'a str,
    },
    ServerScrollBadge {
        offset: usize,
    },
    InternalBridgePortSummary {
        port: u16,
        auto_broadcast: bool,
    },
    CopySuccessFeedback {
        is_public: bool,
    },
    WebLogBarTitle {
        total_lines: usize,
        max_lines: usize,
    },
    WebLogBarScrolled {
        scroll_pos: usize,
    },
}
