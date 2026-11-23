use crate::structs::languages::{AppMsg, AppMsgCode};

#[must_use]
pub fn text(code: AppMsgCode) -> &'static str {
    match code {
        AppMsgCode::AuthorLabel => "ソフト作者:",
        AppMsgCode::AuthorName => "アサネオン",
        AppMsgCode::BrandBuild => "Build",
        AppMsgCode::BrandWith => "With",
        AppMsgCode::BrandAntigravity => "Antigravity",
        AppMsgCode::MissingServerDirError => {
            "本ソフトをサーバーのフォルダの下に入れた後で再起動してください!"
        }
        AppMsgCode::PublicIpLabel => "パブリックIP:",
        AppMsgCode::ShutdownSignalReceived => {
            "終了シグナルを受信しました。サーバーとWebコンソールを停止しています..."
        }
        AppMsgCode::SummaryHeader => {
            "======================= 起動設定まとめ ======================="
        }
        AppMsgCode::SummaryFooter => {
            "=============================================================="
        }
        AppMsgCode::SummaryMapLabel => "マップ:        ",
        AppMsgCode::SummaryServerPortLabel => "サーバーポート:",
        AppMsgCode::SummaryWebConsoleLabel => "Webコンソール: ",
        AppMsgCode::SummaryDefaultModsLabel => "自動注入MOD:   ",
        AppMsgCode::SummarySelectedModsLabel => "選択済みMOD:   ",
        AppMsgCode::NoneLabel => "なし",
        AppMsgCode::ExitedCleanly => "正常に終了しました。",
        AppMsgCode::UnexpectedErrorHeader => "予期しないエラーが発生しました:",
    }
}

#[must_use]
pub fn render(msg: &AppMsg<'_>) -> String {
    match msg {
        AppMsg::PublicIpFetchFailed { error_text } => {
            format!("パブリックIPの取得に失敗しました: {error_text}")
        }
        AppMsg::MissingShippingExecutable { path } => {
            format!("サーバー実行ファイルが見つかりません: {path}")
        }
        AppMsg::LegacyFridaDllDetected {
            file_name,
            full_path,
        } => {
            format!("警告: 旧パッチ用の {file_name} ({full_path}) が検出されました。")
        }
        AppMsg::LegacyFridaDllAdvice { process_name } => {
            format!(
                "原生サーバーで安定動作させるため、未改造の {process_name} を使用し frida.dll / frida.config / Patches を削除してください。"
            )
        }
        AppMsg::ConfigSummary(params) => {
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
        AppMsg::ConfigCreated { relative_path } => {
            format!("デフォルト設定ファイル ({relative_path}) を作成しました")
        }
        AppMsg::ConfigLoadFailed { error_text } => {
            format!(
                "config.json の読み込みに失敗しました。デフォルト設定を使用します: {error_text}"
            )
        }
        AppMsg::WebConsoleStartFailed { port, error_text } => {
            format!("Webコンソールの起動に失敗しました (ポート {port}): {error_text}")
        }
        AppMsg::SummaryInternalPatchPort { port } => {
            format!("(内部パッチポート: {port})")
        }
    }
}
