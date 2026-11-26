use crate::structs::languages::{CliErrorCode, CliMsg, CliMsgCode};

#[must_use]
pub fn text(code: CliMsgCode) -> &'static str {
    match code {
        CliMsgCode::MapLabel1 => "山頂",
        CliMsgCode::MapLabel2 => "広漠",
        CliMsgCode::MapLabel3 => "進入路",
        CliMsgCode::WebConsoleSectionHeader => {
            "-------------------- [Webコンソール設定] --------------------"
        }
        CliMsgCode::ServerSetupSectionHeader => {
            "-------------------- [サーバー・試合設定] --------------------"
        }
        CliMsgCode::ReconfigureSectionHeader => {
            "================== マップ・MOD再設定モード =================="
        }
        CliMsgCode::WebPortPrompt => "Webコンソールに使うTCPポートを入れてください",
        CliMsgCode::AutoBroadcastPromptPrefix => "自動ブロードキャストスイッチ",
        CliMsgCode::AutoBroadcastChoices => "1:オン, 0:オフ",
        CliMsgCode::SwitchPortOnRestartPromptPrefix => "再起動時のポート切替",
        CliMsgCode::SwitchPortOnRestartChoices => "1:切替あり(2ポート交互), 0:固定ポート(1ポート)",
        CliMsgCode::ServerPortPromptPrefix => "サーバーに使うポートを入れてください",
        CliMsgCode::ServerPortsSelectedLabel => "サーバーポート:",
        CliMsgCode::ServerPortSwitchBadge => "(再起動時切替あり)",
        CliMsgCode::ServerPortFixedBadge => "(固定ポート・切替なし)",
        CliMsgCode::MapPromptPrefix => "マップを設定してください",
        CliMsgCode::MapPromptChoices => "1:山頂, 2:広漠, 3:進入路",
        CliMsgCode::ServerConfigPromptPrefix => "サーバーの配置を設定してください",
        CliMsgCode::ServerConfigChoices => "1:デフォルト設定, 2:設定を編集する",
        CliMsgCode::SettingMaxPlayers => "最大人数設定",
        CliMsgCode::SettingDaysBeforeBlizzard => "暴風雪前の日数設定",
        CliMsgCode::SettingDayMinutes => "一日の時間設定",
        CliMsgCode::SettingPredatorDamage => "野生動物ダメージ設定",
        CliMsgCode::SettingColdIntensity => "寒さ強度設定",
        CliMsgCode::SettingHungerRate => "ハングリー強度設定",
        CliMsgCode::SettingCoalBurnRate => "石炭燃焼率設定",
        CliMsgCode::SettingThralls => "傀儡人数設定",
        CliMsgCode::InvalidChoiceRetry => "規格外の選択肢を入力しました、やり直してください!",
        CliMsgCode::InvalidChoiceShort => "規格外の選択肢です!",
        CliMsgCode::ContentModsLabel => {
            "クライアントPAK必須MOD (プレイヤー側へ対応 .pak 導入が必要):"
        }
        CliMsgCode::ContentModsGuideBeforeY => {
            "下記のナンバーでMODを選択してください(カンマ区切りで複数選択可 例: 1 /"
        }
        CliMsgCode::ContentModChoicePrompt => {
            "クライアントPAK必須MODを入力してください (不要ならそのまま Y):"
        }
        CliMsgCode::ContentModPakSuffix => " [PAK必須]",
        CliMsgCode::DefaultModsLabel => "デフォルトMOD(自動注入):",
        CliMsgCode::ServerOnlyModsLabel => "サーバー専用MOD (クライアントPAK不要):",
        CliMsgCode::ModSelectionGuideBeforeY => {
            "下記のナンバーで機能を選択してください(カンマ区切りで複数選択可 例: 1,2,7 /"
        }
        CliMsgCode::ModSelectionGuideBetweenYAndC => "で決定 /",
        CliMsgCode::ModSelectionGuideAfterC => "でクリア):",
        CliMsgCode::ModChoicePrompt => "サーバー専用MODを入力してください:",
        CliMsgCode::ModModeSuffix => " [セット]",
        CliMsgCode::ModConflictLegendPrefix => "同時選択不可(排他):",
        CliMsgCode::ModSelectionCleared => "選択済みのスクリプトをクリアしました。",
        CliMsgCode::ModAddedPrefix => "追加しました:",
        CliMsgCode::ModDuplicatePrefix => "選択済みのためスキップ:",
        CliMsgCode::ModConflictSkippedPrefix => "排他(同時選択不可)のためスキップ:",
    }
}

#[must_use]
pub fn render(msg: &CliMsg<'_>) -> String {
    match msg {
        CliMsg::InvalidChoiceList { items } => format!("規格外の選択肢です: {items}"),
        CliMsg::ModConflictDetail {
            option_text,
            targets,
        } => format!("{option_text} ({targets}と同時選択不可)"),
        CliMsg::ModCurrentSelection { names } => format!("(選択中: {names})"),
        CliMsg::ReconfigurePreservedNotice {
            port_1,
            port_2,
            switch_port_on_restart,
        } => {
            if *switch_port_on_restart {
                format!(
                    "Webコンソールとサーバーポート設定({port_1} <-> {port_2} / 切替あり)はそのまま維持されます"
                )
            } else {
                format!("Webコンソールとサーバーポート設定({port_1} 固定)はそのまま維持されます")
            }
        }
        CliMsg::ReconfigureInterruptedNotice { error_text } => {
            format!("再設定が中断されました ({error_text})。前回設定のまま再起動します。")
        }
    }
}

#[must_use]
pub fn render_error(err: CliErrorCode) -> String {
    match err {
        CliErrorCode::PortOutOfRange { min, max } => {
            format!("無効な入力です。{min}〜{max}の数値を入力してください!")
        }
        CliErrorCode::WebPortConflictsWithPatch { patch_port } => {
            format!(
                "コンソールパッチ内部ポート({patch_port})と重複しています。別のポートを指定してください!"
            )
        }
        CliErrorCode::ServerPortsDuplicate => {
            "1つ目と2つ目のポートは別の番号を指定してください!".to_string()
        }
        CliErrorCode::ServerPortConflictsWithWeb {
            web_port,
            patch_port,
        } => {
            format!(
                "Webコンソール関連ポート({web_port}, {patch_port})と重複しています。別のポートを指定してください!"
            )
        }
    }
}
