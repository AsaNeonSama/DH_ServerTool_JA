use crate::structs::languages::{TerminalMsg, TerminalMsgCode};

#[must_use]
pub fn text(code: TerminalMsgCode) -> &'static str {
    match code {
        TerminalMsgCode::PressEnterToExit => "Enterキーを押して終了してください...",
        TerminalMsgCode::PostMatchCountdownPrefix => "自動再起動まで残り",
        TerminalMsgCode::PostMatchCountdownMid => "秒... (",
        TerminalMsgCode::PostMatchCountdownReconfigure => ": マップ・MOD再設定 /",
        TerminalMsgCode::PostMatchCountdownRestart => ": 今すぐ再起動)",
        TerminalMsgCode::Tab2Title => " [2] サーバー・試合設定 ",
        TerminalMsgCode::TabSwitchHint => "  [←/→・クリック切替]",
        TerminalMsgCode::WebSettingsHeader => {
            "------------------- [Webコンソール設定] -------------------"
        }
        TerminalMsgCode::WebTcpPortLabel => "TCPポート:",
        TerminalMsgCode::WebAutoBroadcastLabel => "/ 自動ブロードキャスト:",
        TerminalMsgCode::ToggleOn => "オン",
        TerminalMsgCode::ToggleOff => "オフ",
        TerminalMsgCode::WebStartedHeader => {
            "==================== Webコンソール起動 ===================="
        }
        TerminalMsgCode::WebStartedFooter => {
            "==========================================================="
        }
        TerminalMsgCode::PublicUrlLabel => "外部URL:",
        TerminalMsgCode::LocalUrlLabel => "内部URL:",
        TerminalMsgCode::PskNoticeLead => "URL末尾の #PSK は事前共有鍵(AES-256-GCM)です。",
        TerminalMsgCode::PskNoticeBody => "全API通信が密文で保護されます。",
        TerminalMsgCode::PskNoticeWarn => "信用していない人に渡さないで下さい",
        TerminalMsgCode::CopyPublicBtnLabel => " 外部URLコピー (P) ",
        TerminalMsgCode::CopyLocalBtnLabel => " 内部URLコピー (L) ",
        TerminalMsgCode::PublicUrlShortLabel => "外部URL",
        TerminalMsgCode::LocalUrlShortLabel => "内部URL",
        TerminalMsgCode::CopyFailureFeedback => {
            " * クリップボードへのコピーに失敗しました (Shift+ドラッグで選択してください)"
        }
        TerminalMsgCode::CopyGuideHint => {
            "ボタンクリックまたは P / L キーでURLをコピー (手動選択は Shift+ドラッグ)"
        }
        TerminalMsgCode::ServerPendingInputHint => {
            "※「サーバー・試合設定」タブで入力・操作待ちです (→キーまたはタブクリックで切替)"
        }
        TerminalMsgCode::SwitchToServerHint => {
            "→ キーまたは「サーバー・試合設定」タブをクリックしてサーバー画面へ切替"
        }
        TerminalMsgCode::WebLogBarControlsHint => " [↑/↓・ホイール・PgUp/PgDn・End] ",
        TerminalMsgCode::WebLogEmptyPlaceholder => "(操作ログはまだありません)",
    }
}

#[must_use]
pub fn render(msg: &TerminalMsg<'_>) -> String {
    match msg {
        TerminalMsg::Tab1Title { port_suffix } => format!(" [1] Webコンソール{port_suffix} "),
        TerminalMsg::ServerScrollBadge { offset } => {
            format!(" ▲ {offset}行上を表示中 (ホイール/PgDnで最新へ) ")
        }
        TerminalMsg::InternalBridgePortSummary {
            port,
            auto_broadcast,
        } => {
            let broadcast_text: &str = if *auto_broadcast {
                text(TerminalMsgCode::ToggleOn)
            } else {
                text(TerminalMsgCode::ToggleOff)
            };
            format!(
                "内部連携ポート(コンソール連携.js): 127.0.0.1:{port} / 自動ブロードキャスト: {broadcast_text}"
            )
        }
        TerminalMsg::CopySuccessFeedback { is_public } => {
            let label: &str = if *is_public {
                text(TerminalMsgCode::PublicUrlShortLabel)
            } else {
                text(TerminalMsgCode::LocalUrlShortLabel)
            };
            format!(" * ✓ {label}をクリップボードにコピーしました!")
        }
        TerminalMsg::WebLogBarTitle {
            total_lines,
            max_lines,
        } => {
            format!(" [操作ログ {total_lines}/{max_lines}] ")
        }
        TerminalMsg::WebLogBarScrolled { scroll_pos } => {
            format!(" ▲ {scroll_pos}行上を表示中 (ホイール/PgDn/Endで最新へ) ")
        }
    }
}
