use crate::structs::languages::{ServerMsg, ServerMsgCode};

#[must_use]
pub fn text(code: ServerMsgCode) -> &'static str {
    match code {
        ServerMsgCode::SpawnedProcessPrefix => "プロセス生成完了 (PID:",
        ServerMsgCode::RoundSummaryRetryTitle => {
            "======================= 起動設定まとめ ======================="
        }
        ServerMsgCode::RoundSummaryUpdatedTitle => {
            "======================= 更新後の設定 ======================="
        }
        ServerMsgCode::RoundSummaryContinuedTitle => {
            "================= 起動設定 (前回設定を継続) ================="
        }
        ServerMsgCode::RoundSummaryFooter => {
            "============================================================"
        }
        ServerMsgCode::RoundMapLabel => "マップ:",
        ServerMsgCode::RoundPortLabel => "ポート:",
        ServerMsgCode::ExitHint => "終了する場合は Ctrl+C を押してください",
        ServerMsgCode::ServerExitDetectedPrefix => "サーバーの終了を検知しました! 次回起動ポート:",
    }
}

#[must_use]
pub fn render(msg: &ServerMsg<'_>) -> String {
    match msg {
        ServerMsg::SpawningSuspended { executable, port } => {
            format!("{executable} を一時停止状態で直接起動中 (ポート: {port})...")
        }
        ServerMsg::PatchInitError {
            file_name,
            error_text,
        } => {
            format!("パッチ初期化エラー ({file_name}): {error_text}")
        }
        ServerMsg::InjectedPatchDone {
            index,
            total,
            label,
        } => {
            format!("[{index}/{total}] 注入完了: {label}")
        }
        ServerMsg::AllPatchesInjectedResumed { count } => {
            format!("全 {count} 件のMODパッチの注入完了 -> サーバー実行を再開しました!")
        }
        ServerMsg::PatchLogHeader {
            timestamp,
            port,
            patch_names,
        } => {
            format!(
                "=== DH_ServerTool_JA Patch Runtime Log ({timestamp} / Port: {port}) ===\nPatches: {patch_names}\n"
            )
        }
        ServerMsg::RetryNotice { error_text } => {
            format!("前回起動失敗のためポート切替再試行 ({error_text})")
        }
        ServerMsg::RoundSummaryCurrentPort { port } => format!("(今回: {port})"),
        ServerMsg::RoundLaunchHeader { round, port } => {
            format!("================= サーバー起動 (#{round} / ポート: {port}) =================")
        }
        ServerMsg::StartupInjectError { error_text } => {
            format!("サーバー起動・注入エラー: {error_text}")
        }
        ServerMsg::ContentModAssetPrepareError { error_text } => {
            format!("クライアントPAK必須MODの資産展開に失敗しました: {error_text}")
        }
        ServerMsg::StartupRetryAfter { delay_text } => {
            format!("{delay_text}後にポートを切り替えて再試行します...")
        }
        ServerMsg::MonitoringServer { interval_text } => {
            format!(
                "サーバー監視中 ({interval_text}間隔 / 終了時に自動ポート切替・再起動します)..."
            )
        }
        ServerMsg::UdpReadyConfirmed { pid, port } => {
            format!("サーバー起動完了確認 (PID: {pid}, UDP:{port} リッスン中)")
        }
        ServerMsg::UdpStoppedRestarting { port } => {
            format!("ポート {port} のリッスン停止を検知しました。サーバーを再起動します...")
        }
    }
}
