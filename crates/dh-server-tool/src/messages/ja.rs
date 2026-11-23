//! 1:1 Japanese message catalog ported from `controller/packages/messages/*`.

pub mod app {
    #[derive(Debug, Clone, Copy)]
    pub struct ConfigSummaryParams<'a> {
        pub server_port_1: u16,
        pub server_port_2: u16,
        pub web_console_port: u16,
        pub console_patch_port: u16,
        pub check_interval_text: &'a str,
        pub restart_wait_seconds: u64,
    }

    pub const AUTHOR_LABEL: &str = "ソフト作者:";
    pub const AUTHOR_NAME: &str = "アサネオン";
    pub const BRAND_BUILD: &str = "Build";
    pub const BRAND_WITH: &str = "With";
    pub const BRAND_ANTIGRAVITY: &str = "Antigravity";
    pub const MISSING_SERVER_DIR_ERROR: &str =
        "本ソフトをサーバーのフォルダの下に入れた後で再起動してください!";
    pub const PUBLIC_IP_LABEL: &str = "パブリックIP:";
    pub const SHUTDOWN_SIGNAL_RECEIVED: &str =
        "終了シグナルを受信しました。サーバーとWebコンソールを停止しています...";
    pub const SUMMARY_HEADER: &str =
        "======================= 起動設定まとめ =======================";
    pub const SUMMARY_FOOTER: &str =
        "==============================================================";
    pub const SUMMARY_MAP_LABEL: &str = "マップ:        ";
    pub const SUMMARY_SERVER_PORT_LABEL: &str = "サーバーポート:";
    pub const SUMMARY_WEB_CONSOLE_LABEL: &str = "Webコンソール: ";
    pub const SUMMARY_DEFAULT_MODS_LABEL: &str = "自動注入MOD:   ";
    pub const SUMMARY_SELECTED_MODS_LABEL: &str = "選択済みMOD:   ";
    pub const NONE_LABEL: &str = "なし";
    pub const EXITED_CLEANLY: &str = "正常に終了しました。";
    pub const UNEXPECTED_ERROR_HEADER: &str = "予期しないエラーが発生しました:";

    #[must_use]
    pub fn public_ip_fetch_failed(error_text: &str) -> String {
        format!("パブリックIPの取得に失敗しました: {error_text}")
    }

    #[must_use]
    pub fn missing_shipping_executable(path: &str) -> String {
        format!("サーバー実行ファイルが見つかりません: {path}")
    }

    #[must_use]
    pub fn legacy_frida_dll_detected(file_name: &str, full_path: &str) -> String {
        format!("警告: 旧パッチ用の {file_name} ({full_path}) が検出されました。")
    }

    #[must_use]
    pub fn legacy_frida_dll_advice(process_name: &str) -> String {
        format!(
            "原生サーバーで安定動作させるため、未改造の {process_name} を使用し frida.dll / frida.config / Patches を削除してください。"
        )
    }

    #[must_use]
    pub fn config_summary(params: &ConfigSummaryParams<'_>) -> String {
        format!(
            "config.json: サーバーポート={}/{}, Webコンソール={}, パッチ連携={}, 監視間隔={}, 再設定待機={}s, 注入方式=Frida直接起動注入(spawn)",
            params.server_port_1,
            params.server_port_2,
            params.web_console_port,
            params.console_patch_port,
            params.check_interval_text,
            params.restart_wait_seconds,
        )
    }

    #[must_use]
    pub fn config_created(relative_path: &str) -> String {
        format!("デフォルト設定ファイル ({relative_path}) を作成しました")
    }

    #[must_use]
    pub fn config_load_failed(error_text: &str) -> String {
        format!("config.json の読み込みに失敗しました。デフォルト設定を使用します: {error_text}")
    }

    #[must_use]
    pub fn web_console_start_failed(port: u16, error_text: &str) -> String {
        format!("Webコンソールの起動に失敗しました (ポート {port}): {error_text}")
    }

    #[must_use]
    pub fn summary_internal_patch_port(port: u16) -> String {
        format!("(内部パッチポート: {port})")
    }
}

pub mod cli {
    pub const MAP_LABEL_1: &str = "山頂";
    pub const MAP_LABEL_2: &str = "広漠";
    pub const MAP_LABEL_3: &str = "進入路";

    #[must_use]
    pub fn map_label(choice: &str) -> Option<&'static str> {
        match choice {
            "1" => Some(MAP_LABEL_1),
            "2" => Some(MAP_LABEL_2),
            "3" => Some(MAP_LABEL_3),
            _ => None,
        }
    }

    pub const WEB_CONSOLE_SECTION_HEADER: &str =
        "-------------------- [Webコンソール設定] --------------------";
    pub const SERVER_SETUP_SECTION_HEADER: &str =
        "-------------------- [サーバー・試合設定] --------------------";
    pub const RECONFIGURE_SECTION_HEADER: &str =
        "================== マップ・MOD再設定モード ==================";
    pub const WEB_PORT_PROMPT: &str = "Webコンソールに使うTCPポートを入れてください";
    pub const AUTO_BROADCAST_PROMPT_PREFIX: &str = "自動ブロードキャストスイッチ";
    pub const AUTO_BROADCAST_CHOICES: &str = "1:オン, 0:オフ";
    pub const SERVER_PORT_PROMPT_PREFIX: &str = "サーバーに使うポートを入れてください";
    pub const SERVER_PORT_DUPLICATE_ERROR: &str =
        "1つ目と2つ目のポートは別の番号を指定してください!";
    pub const SERVER_PORTS_SELECTED_LABEL: &str = "サーバーポート:";
    pub const MAP_PROMPT_PREFIX: &str = "マップを設定してください";
    pub const MAP_PROMPT_CHOICES: &str = "1:山頂, 2:広漠, 3:進入路";
    pub const SERVER_CONFIG_PROMPT_PREFIX: &str = "サーバーの配置を設定してください";
    pub const SERVER_CONFIG_CHOICES: &str = "1:デフォルト設定, 2:設定を編集する";

    pub mod setting_labels {
        pub const MAX_PLAYERS: &str = "最大人数設定";
        pub const DAYS_BEFORE_BLIZZARD: &str = "暴風雪前の日数設定";
        pub const DAY_MINUTES: &str = "一日の時間設定";
        pub const PREDATOR_DAMAGE: &str = "野生動物ダメージ設定";
        pub const COLD_INTENSITY: &str = "寒さ強度設定";
        pub const HUNGER_RATE: &str = "ハングリー強度設定";
        pub const COAL_BURN_RATE: &str = "石炭燃焼率設定";
        pub const THRALLS: &str = "傀儡人数設定";
    }

    pub const INVALID_CHOICE_RETRY: &str = "規格外の選択肢を入力しました、やり直してください!";
    pub const INVALID_CHOICE_SHORT: &str = "規格外の選択肢です!";
    pub const DEFAULT_MODS_LABEL: &str = "デフォルトMOD(自動注入):";
    pub const MOD_SELECTION_GUIDE_BEFORE_Y: &str =
        "下記のナンバーで機能を選択してください(カンマ区切りで複数選択可 例: 1,2,7 /";
    pub const MOD_SELECTION_GUIDE_BETWEEN_Y_AND_C: &str = "で決定 /";
    pub const MOD_SELECTION_GUIDE_AFTER_C: &str = "でクリア):";
    pub const MOD_CHOICE_PROMPT: &str = "選択肢を入力してください:";
    pub const MOD_MODE_SUFFIX: &str = " [セット]";
    pub const MOD_CONFLICT_LEGEND_PREFIX: &str = "同時選択不可(排他):";
    pub const MOD_SELECTION_CLEARED: &str = "選択済みのスクリプトをクリアしました。";
    pub const MOD_ADDED_PREFIX: &str = "追加しました:";
    pub const MOD_DUPLICATE_PREFIX: &str = "選択済みのためスキップ:";
    pub const MOD_CONFLICT_SKIPPED_PREFIX: &str = "排他(同時選択不可)のためスキップ:";

    #[must_use]
    pub fn port_range_error(min: u16, max: u16) -> String {
        format!("無効な入力です。{min}〜{max}の数値を入力してください!")
    }

    #[must_use]
    pub fn web_port_conflict_error(patch_port: u16) -> String {
        format!(
            "コンソールパッチ内部ポート({patch_port})と重複しています。別のポートを指定してください!"
        )
    }

    #[must_use]
    pub fn server_port_web_conflict_error(web_port: u16, patch_port: u16) -> String {
        format!(
            "Webコンソール関連ポート({web_port}, {patch_port})と重複しています。別のポートを指定してください!"
        )
    }

    #[must_use]
    pub fn invalid_choice_list(items: &str) -> String {
        format!("規格外の選択肢です: {items}")
    }

    #[must_use]
    pub fn mod_conflict_detail(option_text: &str, targets: &str) -> String {
        format!("{option_text} ({targets}と同時選択不可)")
    }

    #[must_use]
    pub fn mod_current_selection(names: &str) -> String {
        format!("(選択中: {names})")
    }

    #[must_use]
    pub fn reconfigure_preserved_notice(port_1: u16, port_2: u16) -> String {
        format!("Webコンソールとサーバーポート設定({port_1} <-> {port_2})はそのまま維持されます")
    }

    #[must_use]
    pub fn reconfigure_interrupted_notice(error_text: &str) -> String {
        format!("再設定が中断されました ({error_text})。前回設定のまま再起動します。")
    }
}

pub mod mods {
    pub mod defaults {
        pub const CONSOLE: &str = "Webコンソール連携";
        pub const SPELL_CHARGE_FIX: &str = "スペルチャージ修正";
        pub const DEATH_OFFLINE_ANNOUNCEMENT: &str = "オフラインアナウンス";
        pub const NETWORK_FIX: &str = "ネットワーク接続保護・乱入防止";
    }

    pub mod selectable {
        pub const RAND_ROLE: &str = "役職ランダム";
        pub const SPELL_CAST_START: &str = "開幕沈黙";
        pub const RESOURCES_SPAWNER_DEPARTURE: &str = "マップ補強(山頂)";
        pub const RESOURCES_SPAWNER_EXPANSE: &str = "マップ補強(広漠)";
        pub const SPELL_CUSTOM: &str = "五サボタージュ";
        pub const MADMATE_NORMAL_NOLIMIT: &str = "狂人モード";
        pub const BONE_DAGGER_MODE: &str = "骨の短剣モード";
        pub const QUEST_SYSTEM: &str = "クエスト報酬";
        pub const ADD_STORAGE: &str = "全員バックパック";
        pub const WHETSTONE_ANY_ITEM: &str = "万能砥石";
        pub const ARROW_FULL_RECOVERY: &str = "矢回収100%";
        pub const BLIZZARD_ARMORY_OPEN: &str = "暴風雪マスケット";
        pub const GUN_AND_BOW: &str = "銃・弓同時所持";
        pub const START_ARMORY_OPEN: &str = "開幕マスケット";
        pub const VOMIT_MODIFY: &str = "毒耐性";
        pub const NORTH_GHOST: &str = "ミステリーモード";
        pub const GRENADE: &str = "グレネード";
        pub const MOON_JUMP: &str = "八艘飛び";
        pub const SPELL_NO_CD: &str = "スペルチャージ無限";
        pub const NO_DEATH: &str = "無限復活";
        pub const SIGHTSEEING_DEPARTURE: &str = "観光モード(山頂)";
        pub const SIGHTSEEING_EXPANSE: &str = "観光モード(広漠)";

        pub mod bone_dagger_scripts {
            pub const MAIN: &str = "骨の短剣モード (短剣本体)";
            pub const DROP_TABLE: &str = "骨の短剣モード (ドロップテーブル)";
            pub const INVENTORY: &str = "骨の短剣モード (初期装備設定)";
        }
    }
}

pub mod server {
    pub const INJECTION_ABORTED: &str = "注入処理が中断されました";
    pub const ATTACH_ABORTED: &str = "Fridaアタッチが中断されました";
    pub const ATTACH_FAILED_FALLBACK: &str = "Frida attach failed";
    pub const LAUNCH_ABORTED: &str = "サーバー起動が中断されました";
    pub const SPAWN_LABEL: &str = "サーバープロセス生成";
    pub const SPAWNED_PROCESS_PREFIX: &str = "プロセス生成完了 (PID:";
    pub const SPAWNED_PROCESS_SUFFIX: &str = ") — パッチ注入を開始します";
    pub const CONSOLE_BOOTSTRAP_CREATE_LABEL: &str = "コンソール初期化スクリプト生成";
    pub const CONSOLE_BOOTSTRAP_LOAD_LABEL: &str = "コンソール初期化スクリプト実行";
    pub const CONSOLE_BOOTSTRAP_UNLOAD_LABEL: &str = "コンソール初期化スクリプト解放";
    pub const PATCH_INJECTION_ABORTED: &str = "パッチ注入が中断されました";
    pub const RESUME_ABORTED: &str = "サーバー再開が中断されました";
    pub const SCRIPT_UNLOAD_LABEL: &str = "スクリプト解放";
    pub const SESSION_DETACH_LABEL: &str = "セッション切断";
    pub const PROCESS_KILL_LABEL: &str = "プロセス終了";
    pub const ROUND_SUMMARY_RETRY_TITLE: &str =
        "======================= 起動設定まとめ =======================";
    pub const ROUND_SUMMARY_UPDATED_TITLE: &str =
        "======================= 更新後の設定 =======================";
    pub const ROUND_SUMMARY_CONTINUED_TITLE: &str =
        "================= 起動設定 (前回設定を継続) =================";
    pub const ROUND_SUMMARY_FOOTER: &str =
        "============================================================";
    pub const ROUND_MAP_LABEL: &str = "マップ:";
    pub const ROUND_PORT_LABEL: &str = "ポート:";
    pub const EXIT_HINT: &str = "終了する場合は Ctrl+C を押してください";
    pub const SERVER_EXIT_DETECTED_PREFIX: &str = "サーバーの終了を検知しました! 次回起動ポート:";

    #[must_use]
    pub fn op_aborted(label: &str) -> String {
        format!("{label} が中断されました")
    }

    #[must_use]
    pub fn op_timed_out(label: &str, timeout_ms: u64) -> String {
        format!("{label} がタイムアウトしました ({timeout_ms}ms)")
    }

    #[must_use]
    pub fn op_failed(label: &str) -> String {
        format!("{label} に失敗しました")
    }

    #[must_use]
    pub fn attach_label(pid: u32) -> String {
        format!("Fridaアタッチ (PID: {pid})")
    }

    #[must_use]
    pub fn spawning_suspended(executable: &str, port: u16) -> String {
        format!("{executable} を一時停止状態で直接起動中 (ポート: {port})...")
    }

    #[must_use]
    pub fn process_detached_during_injection(reason: &str) -> String {
        format!("プロセスから切断されたため注入を中断しました ({reason})")
    }

    #[must_use]
    pub fn injecting_patch_prefix(index: usize, total: usize) -> String {
        format!("[{index}/{total}] 注入中:")
    }

    #[must_use]
    pub fn script_create_label(file_name: &str) -> String {
        format!("スクリプト生成 ({file_name})")
    }

    #[must_use]
    pub fn script_load_label(file_name: &str) -> String {
        format!("スクリプト注入 ({file_name})")
    }

    #[must_use]
    pub fn patch_init_error(file_name: &str, error_text: &str) -> String {
        format!("パッチ初期化エラー ({file_name}): {error_text}")
    }

    #[must_use]
    pub fn injected_patch_done(index: usize, total: usize, label: &str) -> String {
        format!("[{index}/{total}] 注入完了: {label}")
    }

    #[must_use]
    pub fn resume_label(pid: u32) -> String {
        format!("サーバー再開 (PID: {pid})")
    }

    #[must_use]
    pub fn all_patches_injected_resumed(count: usize) -> String {
        format!("全 {count} 件のMODパッチの注入完了 -> サーバー実行を再開しました!")
    }

    #[must_use]
    pub fn patch_log_header(timestamp: &str, port: u16, patch_names: &str) -> String {
        format!(
            "=== DH_ServerTool_JA Patch Runtime Log ({timestamp} / Port: {port}) ===\nPatches: {patch_names}\n"
        )
    }

    #[must_use]
    pub fn retry_notice(error_text: &str) -> String {
        format!("前回起動失敗のためポート切替再試行 ({error_text})")
    }

    #[must_use]
    pub fn round_summary_current_port(port: u16) -> String {
        format!("(今回: {port})")
    }

    #[must_use]
    pub fn round_launch_header(round: u32, port: u16) -> String {
        format!("================= サーバー起動 (#{round} / ポート: {port}) =================")
    }

    #[must_use]
    pub fn frida_detached(reason: &str) -> String {
        format!("Fridaセッション切断を検知しました ({reason})")
    }

    #[must_use]
    pub fn startup_inject_error(error_text: &str) -> String {
        format!("サーバー起動・注入エラー: {error_text}")
    }

    #[must_use]
    pub fn startup_retry_after(delay_text: &str) -> String {
        format!("{delay_text}後にポートを切り替えて再試行します...")
    }

    #[must_use]
    pub fn monitoring_server(interval_text: &str) -> String {
        format!("サーバー監視中 ({interval_text}間隔 / 終了時に自動ポート切替・再起動します)...")
    }

    #[must_use]
    pub fn udp_ready_confirmed(pid: u32, port: u16) -> String {
        format!("サーバー起動完了確認 (PID: {pid}, UDP:{port} リッスン中)")
    }

    #[must_use]
    pub fn udp_stopped_restarting(port: u16) -> String {
        format!("ポート {port} のリッスン停止を検知しました。サーバーを再起動します...")
    }
}

pub mod terminal {
    pub const PRESS_ENTER_TO_EXIT: &str = "Enterキーを押して終了してください...";
    pub const POST_MATCH_COUNTDOWN_PREFIX: &str = "自動再起動まで残り";
    pub const POST_MATCH_COUNTDOWN_MID: &str = "秒... (";
    pub const POST_MATCH_COUNTDOWN_RECONFIGURE: &str = ": マップ・MOD再設定 /";
    pub const POST_MATCH_COUNTDOWN_RESTART: &str = ": 今すぐ再起動)";
    pub const TAB2_TITLE: &str = " [2] サーバー・試合設定 ";
    pub const TAB_SWITCH_HINT: &str = "  [←/→・クリック切替]";
    pub const WEB_SETTINGS_HEADER: &str =
        "------------------- [Webコンソール設定] -------------------";
    pub const WEB_TCP_PORT_LABEL: &str = "TCPポート:";
    pub const WEB_AUTO_BROADCAST_LABEL: &str = "/ 自動ブロードキャスト:";
    pub const TOGGLE_ON: &str = "オン";
    pub const TOGGLE_OFF: &str = "オフ";
    pub const WEB_STARTED_HEADER: &str =
        "==================== Webコンソール起動 ====================";
    pub const WEB_STARTED_FOOTER: &str =
        "===========================================================";
    pub const PUBLIC_URL_LABEL: &str = "外部URL:";
    pub const LOCAL_URL_LABEL: &str = "内部URL:";
    pub const PSK_NOTICE_LEAD: &str = "URL末尾の #PSK は事前共有鍵(AES-256-GCM)です。";
    pub const PSK_NOTICE_BODY: &str = "全API通信が密文で保護されます。";
    pub const PSK_NOTICE_WARN: &str = "信用していない人に渡さないで下さい";
    pub const COPY_PUBLIC_BTN_LABEL: &str = " 外部URLコピー (P) ";
    pub const COPY_LOCAL_BTN_LABEL: &str = " 内部URLコピー (L) ";
    pub const PUBLIC_URL_SHORT_LABEL: &str = "外部URL";
    pub const LOCAL_URL_SHORT_LABEL: &str = "内部URL";
    pub const COPY_FAILURE_FEEDBACK: &str =
        " * クリップボードへのコピーに失敗しました (Shift+ドラッグで選択してください)";
    pub const COPY_LOG_CATEGORY: &str = "コピー";
    pub const COPY_GUIDE_HINT: &str =
        "ボタンクリックまたは P / L キーでURLをコピー (手動選択は Shift+ドラッグ)";
    pub const SERVER_PENDING_INPUT_HINT: &str =
        "※「サーバー・試合設定」タブで入力・操作待ちです (→キーまたはタブクリックで切替)";
    pub const SWITCH_TO_SERVER_HINT: &str =
        "→ キーまたは「サーバー・試合設定」タブをクリックしてサーバー画面へ切替";
    pub const WEB_LOG_BAR_CONTROLS_HINT: &str = " [↑/↓・ホイール・PgUp/PgDn・End] ";
    pub const WEB_LOG_EMPTY_PLACEHOLDER: &str = "(操作ログはまだありません)";

    #[must_use]
    pub fn tab1_title(port_suffix: &str) -> String {
        format!(" [1] Webコンソール{port_suffix} ")
    }

    #[must_use]
    pub fn server_scroll_badge(offset: usize) -> String {
        format!(" ▲ {offset}行上を表示中 (ホイール/PgDnで最新へ) ")
    }

    #[must_use]
    pub fn internal_bridge_port_summary(port: u16, broadcast_text: &str) -> String {
        format!(
            "内部連携ポート(console_win64.js): 127.0.0.1:{port} / 自動ブロードキャスト: {broadcast_text}"
        )
    }

    #[must_use]
    pub fn copy_success_feedback(label: &str) -> String {
        format!(" * ✓ {label}をクリップボードにコピーしました!")
    }

    #[must_use]
    pub fn copy_log_success(label: &str) -> String {
        format!("{label}をクリップボードにコピーしました")
    }

    #[must_use]
    pub fn copy_log_failure(label: &str) -> String {
        format!("{label}のクリップボードコピーに失敗")
    }

    #[must_use]
    pub fn web_log_bar_title(total_lines: usize, max_lines: usize) -> String {
        format!(" [操作ログ {total_lines}/{max_lines}] ")
    }

    #[must_use]
    pub fn web_log_bar_scrolled(scroll_pos: usize) -> String {
        format!(" ▲ {scroll_pos}行上を表示中 (ホイール/PgDn/Endで最新へ) ")
    }
}

pub mod web_console {
    pub const PLAYER_NAME_LIST: [&str; 8] = [
        "Captain",
        "Chaplain",
        "Cook",
        "Doctor",
        "Engineer",
        "Hunter",
        "Marine",
        "Navigator",
    ];

    #[must_use]
    pub fn player_role_name(role: &str) -> Option<&'static str> {
        match role {
            "Captain" => Some("船長"),
            "Chaplain" => Some("牧師"),
            "Cook" => Some("料理人"),
            "Doctor" => Some("医者"),
            "Engineer" => Some("エンジニア"),
            "Hunter" => Some("猟師"),
            "Marine" => Some("海兵"),
            "Navigator" => Some("航海士"),
            _ => None,
        }
    }

    #[must_use]
    pub fn actor_display_name(key: &str) -> Option<&'static str> {
        match key {
            "BP_Prey_Rat_C" => Some("ネズミ"),
            "BP_Prey_Rabbit_C" => Some("ウサギ"),
            "BP_Prey_Seal_C" => Some("アザラシ"),
            "BP_Predator_Wolf_C" => Some("狼"),
            "BP_Predator_Bear_C" => Some("熊"),
            "BP_Cannibal_C" | "BP_Cannibal_Crawler_C" | "BP_Cannibal_Shambler_C" => {
                Some("人喰い人種")
            }
            "BP_HumanCharacter_C" => Some("海兵"),
            "BP_Totem_C" => Some("トーテム"),
            "BP_ExplosionActor_C" => Some("爆裂魔法"),
            "BP_Explosion_Nitro_C" => Some("大爆裂魔法・チャージ 120 %"),
            "/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Inventory.BP_Flintlock_Inventory_C" => {
                Some("拳銃")
            }
            "/Game/Blueprints/Inventory/Musket/BP_Musket_Inventory.BP_Musket_Inventory_C" => {
                Some("マスケット")
            }
            "/Game/Blueprints/Inventory/Bow/BP_Bow_Inventory.BP_Bow_Inventory_C" => Some("弓"),
            "/Game/Blueprints/Inventory/Flint/BP_GunParts_Inventory.BP_GunParts_Inventory_C" => {
                Some("拳銃パーツ")
            }
            "/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Ammo_Inventory.BP_Flintlock_Ammo_Inventory_C" => {
                Some("銃弾")
            }
            "/Game/Blueprints/Inventory/Bow/BP_Arrows_Inventory.BP_Arrows_Inventory_C" => {
                Some("弓矢")
            }
            "/Game/Blueprints/Inventory/Sword/BP_Sword_Inventory.BP_Sword_Inventory_C" => {
                Some("サーベル")
            }
            "/Game/Blueprints/Inventory/WoodAxe/BP_WoodAxe_Inventory.BP_WoodAxe_Inventory_C" => {
                Some("木の斧")
            }
            "/Game/Blueprints/Inventory/IceAxe/BP_IceAxe_Inventory.BP_IceAxe_Inventory_C" => {
                Some("ピッケル")
            }
            "/Game/Blueprints/Inventory/Cleaver/BP_Cleaver_Inventory.BP_Cleaver_Inventory_C" => {
                Some("包丁")
            }
            "/Game/Blueprints/Inventory/Shovel/BP_Shovel_Inventory.BP_Shovel_Inventory_C" => {
                Some("シャベル")
            }
            "/Game/Blueprints/Inventory/Stick/BP_Stick_Inventory.BP_Stick_Inventory_C" => {
                Some("木")
            }
            "/Game/Blueprints/Inventory/Rock/BP_Rock_Inventory.BP_Rock_Inventory_C" => {
                Some("石")
            }
            "/Game/Blueprints/Inventory/Metals/BP_IronIngot_Inventory.BP_IronIngot_Inventory_C" => {
                Some("スクラップ")
            }
            "/Game/Blueprints/Inventory/Metals/BP_Nails_Inventory.BP_Nails_Inventory_C" => {
                Some("釘")
            }
            "/Game/Blueprints/Inventory/Metals/BP_LeadIngot_Inventory.BP_LeadIngot_Inventory_C" => {
                Some("鉛")
            }
            "/Game/Blueprints/Inventory/Coal/BP_Coal_Inventory.BP_Coal_Inventory_C" => {
                Some("石炭")
            }
            "/Game/Blueprints/Inventory/Flintlock/BP_Gunpowder_Inventory.BP_Gunpowder_Inventory_C" => {
                Some("火薬")
            }
            "/Game/Blueprints/Inventory/AnimalParts/BP_Sinew_Inventory.BP_Sinew_Inventory_C" => {
                Some("腱")
            }
            "/Game/Blueprints/Inventory/AnimalParts/BP_WolfPelt_Inventory.BP_WolfPelt_Inventory_C" => {
                Some("毛皮")
            }
            "/Game/Blueprints/Inventory/Tea/BP_Herbs_Inventory.BP_Herbs_Inventory_C" => {
                Some("ドライハーブ")
            }
            "/Game/Blueprints/Inventory/Syringe/BP_Syringe_Inventory.BP_Syringe_Inventory_C" => {
                Some("注射器")
            }
            "/Game/Blueprints/Inventory/Poison/BP_Antidote_Inventory.BP_Antidote_Inventory_C" => {
                Some("解毒剤")
            }
            "/Game/Blueprints/Inventory/Syringe/BP_Inventory_Laudanum.BP_Inventory_Laudanum_C" => {
                Some("アヘンチンキ")
            }
            "/Game/Blueprints/Inventory/Meat/BP_AnimalMeat_Inventory.BP_AnimalMeat_Inventory_C" => {
                Some("動物肉")
            }
            "/Game/Blueprints/Inventory/Meat/BP_CookedMeat_Inventory.BP_CookedMeat_Inventory_C" => {
                Some("焼肉")
            }
            "/Game/Blueprints/Inventory/Meat/BP_HumanMeat_Inventory.BP_HumanMeat_Inventory_C" => {
                Some("人肉")
            }
            "/Game/Blueprints/Inventory/Meat/BP_BoneClub_Inventory.BP_BoneClub_Inventory_C" => {
                Some("骨の棒")
            }
            "/Game/Blueprints/Inventory/AnimalParts/BP_Blubber_Inventory.BP_Blubber_Inventory_C" => {
                Some("脂")
            }
            "/Game/Blueprints/Inventory/Meat/BP_Stew_Inventory.BP_Stew_Inventory_C" => {
                Some("シチュー")
            }
            "/Game/Blueprints/Inventory/Tea/BP_Tea_Inventory.BP_Tea_Inventory_C" => Some("お茶"),
            "/Game/Blueprints/Inventory/Metals/BP_Whetstone_Inventory.BP_Whetstone_Inventory_C" => {
                Some("砥石")
            }
            "/Game/Blueprints/Inventory/BearTrap/BP_BearTrap_Inventory.BP_BearTrap_Inventory_C" => {
                Some("熊罠")
            }
            "/Game/Blueprints/Inventory/Spyglass/BP_Spyglass_Inventory.BP_Spyglass_Inventory_C" => {
                Some("望遠鏡")
            }
            "/Game/Blueprints/Inventory/Lantern/BP_Lantern_Inventory.BP_Lantern_Inventory_C" => {
                Some("ランタン")
            }
            "/Game/Blueprints/Inventory/Powderkeg/BP_CoalBarrel_Inventory.BP_CoalBarrel_Inventory_C" => {
                Some("石炭樽")
            }
            "/Game/Blueprints/Inventory/Powderkeg/BP_Powderkeg_Inventory.BP_Powderkeg_Inventory_C" => {
                Some("火薬樽")
            }
            "/Game/Blueprints/Inventory/Poison/BP_Poison_Inventory.BP_Poison_Inventory_C" => {
                Some("毒薬")
            }
            "/Game/Blueprints/Environment/Nitro/BP_Nitro_Inventory.BP_Nitro_Inventory_C" => {
                Some("ニトログリセリン")
            }
            "/Game/Blueprints/Inventory/LockPick/BP_SkeletonKey_Inventory.BP_SkeletonKey_Inventory_C" => {
                Some("骨の鍵")
            }
            "/Game/Blueprints/Inventory/LockPick/BP_CaptainsKey_Inventory.BP_CaptainsKey_Inventory_C" => {
                Some("船長室の鍵")
            }
            "/Game/Blueprints/Inventory/Armory/BP_Code_Inventory.BP_Code_Inventory_C" => {
                Some("武器庫の暗号")
            }
            "/Game/Blueprints/Inventory/Quest/BP_Quest_Inventory.BP_Quest_Inventory_C" => {
                Some("任務の巻物")
            }
            "/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C" => {
                Some("骨の短剣")
            }
            _ => None,
        }
    }

    pub mod api {
        pub const SUCCESS: &str = "成功しました!";
        pub const NETWORK_PARAM_ERROR: &str =
            "ネットワーク問題で引数を取得できませんでした。もう一度試してください!";
        pub const SERVER_EXECUTION_ERROR: &str =
            "サーバー側で問題が発生しました。試合開始後にもう一度試すか、サーバーの再起動・JSパッチをご確認ください!";
        pub const INVALID_VALUE_ERROR: &str = "数値エラー!";
        pub const HIGH_VALUE_WARNING: &str = "送信成功! 数値が高すぎるとエラーの可能性があります!";
        pub const BROADCAST_PARTIAL_ERROR: &str =
            "送信成功しましたが、自動ブロードキャストに失敗しました!";
    }

    pub mod log_categories {
        pub const AUTH: &str = "認証";
        pub const ERROR: &str = "エラー";
        pub const COMMAND: &str = "コマンド";
        pub const STATUS: &str = "ステータス";
        pub const ITEM: &str = "アイテム";
        pub const STORAGE: &str = "ストレージ";
        pub const NOTICE: &str = "通知";
        pub const BROADCAST: &str = "全体通知";
        pub const KICK: &str = "キック";
        pub const STARTUP: &str = "起動";
        pub const SHUTDOWN: &str = "停止";
    }

    pub mod result_badges {
        pub const SENT: &str = "[送信済]";
        pub const PARTIAL_FAILURE: &str = "[一部送信失敗]";

        #[must_use]
        pub fn send_failed(suffix: &str) -> String {
            format!("[送信失敗{suffix}]")
        }

        #[must_use]
        pub fn invalid(suffix: &str) -> String {
            format!("[無効{suffix}]")
        }
    }

    pub mod log_details {
        pub const INVALID_PARAMS: &str = "パラメータ不正";
        pub const INVALID_NUMBER_REASON: &str = "不正な数値";
        pub const INVALID_COUNT_REASON: &str = "不正な個数";
        pub const INVALID_VALUE_REASON: &str = "不正な値";
        pub const BONE_DAGGER_ADDED_SUFFIX: &str = " (+骨の短剣)";
        pub const STORAGE_CAPACITY_LABEL: &str = "容量 ->";
        pub const BROADCAST_ALL_TARGET: &str = "全員";
        pub const KICK_VERB: &str = "をキック";
        pub const SHUTDOWN_DETAIL: &str = "Webコンソールを停止しました";

        #[must_use]
        pub fn unauthenticated_rejected(client_ip: &str, pathname: &str) -> String {
            format!("{client_ip} | 未認証リクエスト拒否 ({pathname})")
        }

        #[must_use]
        pub fn invalid_request_body(client_ip: &str, pathname: &str) -> String {
            format!("{client_ip} | リクエスト本体不正 ({pathname})")
        }

        #[must_use]
        pub fn decrypt_failed(client_ip: &str, pathname: &str) -> String {
            format!("{client_ip} | 暗号復号失敗 ({pathname})")
        }

        #[must_use]
        pub fn invalid_json_params(client_ip: &str, pathname: &str) -> String {
            format!("{client_ip} | 不正なJSONパラメータ ({pathname})")
        }

        #[must_use]
        pub fn internal_route_error(client_ip: &str, pathname: &str, msg: &str) -> String {
            format!("{client_ip} | 内部エラー ({pathname}): {msg}")
        }

        #[must_use]
        pub fn login_failed_invalid_body(client_ip: &str) -> String {
            format!("{client_ip} | ログイン失敗 (リクエスト不正)")
        }

        #[must_use]
        pub fn login_failed_psk_mismatch(client_ip: &str) -> String {
            format!("{client_ip} | ログイン失敗 (PSK不一致)")
        }

        #[must_use]
        pub fn login_failed_challenge_invalid(client_ip: &str) -> String {
            format!("{client_ip} | ログイン失敗 (チャレンジ期限切れ/無効)")
        }

        #[must_use]
        pub fn login_succeeded(client_ip: &str) -> String {
            format!("{client_ip} | ログイン成功")
        }

        #[must_use]
        pub fn login_failed_internal(client_ip: &str) -> String {
            format!("{client_ip} | ログイン失敗 (内部エラー)")
        }

        #[must_use]
        pub fn startup_detail(web_port: u16, patch_port: u16, broadcast_text: &str) -> String {
            format!(
                "Webコンソール待機開始 (ポート: {web_port} / 内部連携: 127.0.0.1:{patch_port} / 自動ブロードキャスト: {broadcast_text})"
            )
        }
    }

    pub mod broadcasts {
        #[must_use]
        pub fn summon(role_name: &str, target: &str) -> String {
            format!("{role_name} が{target}を召喚しました")
        }

        #[must_use]
        pub fn unset_thrall(role_name: &str) -> String {
            format!("{role_name} が船員に戻りました")
        }

        #[must_use]
        pub fn set_thrall(role_name: &str) -> String {
            format!("{role_name} が闇の傀儡に悪墜ちしました")
        }

        #[must_use]
        pub fn spell_charge(role_name: &str) -> String {
            format!("{role_name} がスペールチャージしました")
        }

        #[must_use]
        pub fn stop_blizzard(role_name: &str) -> String {
            format!("{role_name} が暴風雪を停止しました")
        }

        #[must_use]
        pub fn start_blizzard(role_name: &str) -> String {
            format!("{role_name} が暴風雪を呼びました")
        }

        #[must_use]
        pub fn reset_time_scale(role_name: &str) -> String {
            format!("{role_name} が世界を元に戻しました")
        }

        #[must_use]
        pub fn accelerate_time_scale(role_name: &str) -> String {
            format!("{role_name} が世界を加速しました")
        }

        #[must_use]
        pub fn disable_god_mode(role_name: &str) -> String {
            format!("{role_name} が無敵状態を解除しました")
        }

        #[must_use]
        pub fn enable_god_mode(role_name: &str) -> String {
            format!("{role_name} が無敵状態になりました")
        }

        #[must_use]
        pub fn modified_stats(role_name: &str) -> String {
            format!("{role_name} が自分のステータスを操作しました")
        }

        #[must_use]
        pub fn acquired_item(role_name: &str, item_label: &str, count: i64) -> String {
            format!("{role_name} が {item_label} {count} を獲得しました")
        }

        #[must_use]
        pub fn storage_limit_changed(role_name: &str, limit: i64) -> String {
            format!("{role_name} のストレージ容量が {limit} になりました")
        }

        #[must_use]
        pub fn kicked(role_name: &str) -> String {
            format!("{role_name} がキックされました")
        }
    }
}
