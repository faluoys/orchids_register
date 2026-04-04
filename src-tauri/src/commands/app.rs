use tauri::{AppHandle, Manager, State};

use crate::state::AppState;

#[tauri::command]
pub async fn cancel_close_prompt(state: State<'_, AppState>) -> Result<(), String> {
    state.cancel_close_prompt();
    Ok(())
}

#[tauri::command]
pub async fn confirm_exit(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.allow_exit();
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.close().map_err(|err| err.to_string())
}
