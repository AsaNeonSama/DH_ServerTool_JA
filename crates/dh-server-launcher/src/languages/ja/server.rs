use crate::structs::languages::{ServerMsg, ServerMsgCode};

#[must_use]
pub fn text(code: ServerMsgCode) -> &'static str {
    match code {
        ServerMsgCode::MapLabel1 => "山頂",
        ServerMsgCode::MapLabel2 => "広漠",
        ServerMsgCode::MapLabel3 => "進入路",
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
                "=== DH_ServerLauncher_JA Patch Runtime Log ({timestamp} / Port: {port}) ===\nPatches: {patch_names}\n"
            )
        }
        ServerMsg::RoundLaunchHeader { round, port } => {
            format!("================= サーバー起動 (#{round} / ポート: {port}) =================")
        }
        ServerMsg::StartupInjectError { error_text } => {
            format!("サーバー起動・注入エラー: {error_text}")
        }
        ServerMsg::ContentModAssetPrepareError { error_text } => {
            format!("クライアント必須MODの資産展開に失敗しました: {error_text}")
        }
        ServerMsg::StartupRetryAfter {
            delay_text,
            switch_port_on_restart,
        } => {
            if *switch_port_on_restart {
                format!("{delay_text}後にポートを切り替えて再試行します...")
            } else {
                format!("{delay_text}後に同じポートで再試行します...")
            }
        }
        ServerMsg::MonitoringServer {
            interval_text,
            switch_port_on_restart,
        } => {
            if *switch_port_on_restart {
                format!(
                    "サーバー監視中 ({interval_text}間隔 / 終了時に自動ポート切替・再起動します)..."
                )
            } else {
                format!(
                    "サーバー監視中 ({interval_text}間隔 / 終了時に同ポートで自動再起動します)..."
                )
            }
        }
        ServerMsg::UdpReadyConfirmed { pid, port } => {
            format!("サーバー起動完了確認 (PID: {pid}, UDP:{port} リッスン中)")
        }
        ServerMsg::UdpStoppedRestarting { port } => {
            format!("ポート {port} のリッスン停止を検知しました。サーバーを再起動します...")
        }
    }
}
