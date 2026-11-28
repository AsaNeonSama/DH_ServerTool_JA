use crate::structs::languages::AppMsg;

#[must_use]
pub fn render(msg: &AppMsg<'_>) -> String {
    match msg {
        AppMsg::ConfigCreated { relative_path } => {
            format!("デフォルト設定ファイル ({relative_path}) を作成しました")
        }
        AppMsg::ConfigLoadFailed { error_text } => {
            format!(
                "launcher_config.json の読み込みに失敗しました。デフォルト設定を使用します: {error_text}"
            )
        }
    }
}
