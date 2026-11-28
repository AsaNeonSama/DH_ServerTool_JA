use crate::structs::languages::{GuiMsg, GuiMsgCode};

#[must_use]
pub fn text(code: GuiMsgCode) -> &'static str {
    match code {
        GuiMsgCode::ServerAlreadyRunning => "サーバーはすでに稼働中または起動処理中です。",
        GuiMsgCode::ServerNotRunning => "現在稼働中のサーバーはありません。",
        GuiMsgCode::InvalidMapSelection => "無効なマップが選択されています。",
        GuiMsgCode::ServerStoppedCleanly => "サーバーを停止しました。",
        GuiMsgCode::WatchdogOffStopped => {
            "サーバー終了を検知しました（自動再起動ウォッチドッグOFFのため停止状態へ移行）"
        }
        GuiMsgCode::ImmediateRestartRequested => "即時再起動コマンドを受信しました。",
        GuiMsgCode::CountdownCancelledForSetup => {
            "自動再起動カウントダウンをキャンセルしました。マップ・MODを変更して再起動できます。"
        }
        GuiMsgCode::NextRoundPlanUpdated => "次回ラウンドのマップ・MOD設定を更新しました。",
        GuiMsgCode::LogDirResolveFailed => "ログフォルダのパス取得に失敗しました。",
    }
}

#[must_use]
pub fn render(msg: &GuiMsg<'_>) -> String {
    match msg {
        GuiMsg::SettingOutOfRange { name, min, max } => {
            format!("{name} は {min}〜{max} の範囲で指定してください。")
        }
        GuiMsg::ServerExeNotFound { path } => {
            format!("サーバー実行ファイルが見つかりません: {path}")
        }
        GuiMsg::OpenFolderFailed { error_text } => {
            format!("ログフォルダを開けませんでした: {error_text}")
        }
    }
}
