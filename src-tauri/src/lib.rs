mod commands;
mod db;
mod orchids_profile;
mod state;

use state::AppState;
use tauri::Manager;

pub fn run() {
    let app_state = AppState::new().expect("数据库初始化失败");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|app| {
            let icon_bytes = include_bytes!("../icons/icon.png");
            let icon = tauri::image::Image::from_bytes(icon_bytes)?;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_icon(icon);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::register::start_registration,
            commands::register::start_batch_registration,
            commands::register::cancel_batch,
            commands::accounts::get_accounts,
            commands::accounts::refresh_accounts_profile_missing,
            commands::accounts::refresh_account_profile,
            commands::accounts::delete_account,
            commands::accounts::delete_accounts,
            commands::accounts::export_accounts,
            commands::accounts::list_account_groups,
            commands::accounts::create_account_group,
            commands::accounts::rename_account_group,
            commands::accounts::delete_account_group,
            commands::accounts::set_account_group_pinned,
            commands::accounts::move_account_group,
            commands::accounts::move_accounts_to_group,
            commands::accounts::save_text_file,
            commands::config::get_all_config,
            commands::config::save_config,
            commands::config::reset_config,
            commands::config::test_proxy,
            commands::config::test_mail_gateway_health,
            commands::domains::list_domains,
            commands::domains::create_domain,
            commands::domains::update_domain,
            commands::domains::delete_domain,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}
