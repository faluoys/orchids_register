use std::collections::HashMap;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::db;
use crate::service_manager::{
    build_mail_gateway_probe_target, build_mail_gateway_spec, build_turnstile_solver_probe_target,
    build_turnstile_solver_spec, repo_root, ServiceManager, ServiceStatus,
};
use crate::state::AppState;

const SERVICE_STATUS_UPDATED_EVENT: &str = "service-status-updated";

#[derive(Clone, Serialize)]
struct ServiceStatusChangedEvent {
    service: &'static str,
    status: ServiceStatus,
}

fn emit_service_status(
    app: &AppHandle,
    service: &'static str,
    status: &ServiceStatus,
) {
    let _ = app.emit_to(
        "main",
        SERVICE_STATUS_UPDATED_EVENT,
        ServiceStatusChangedEvent {
            service,
            status: status.clone(),
        },
    );
}

fn load_config(state: &State<'_, AppState>) -> Result<HashMap<String, String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_all_config(&conn).map_err(|e| e.to_string())
}

fn repo_root_string() -> Result<String, String> {
    Ok(repo_root()?.to_string_lossy().into_owned())
}

pub fn get_service_status_inner(
    manager: &mut ServiceManager,
    config: Option<&HashMap<String, String>>,
) -> HashMap<String, ServiceStatus> {
    let mail_gateway_target = config.and_then(|items| build_mail_gateway_probe_target(items).ok());
    let turnstile_solver_target =
        config.and_then(|items| build_turnstile_solver_probe_target(items).ok());
    manager.get_status_map_with_targets(mail_gateway_target, turnstile_solver_target)
}

#[tauri::command]
pub async fn get_service_status(
    state: State<'_, AppState>,
) -> Result<HashMap<String, ServiceStatus>, String> {
    let config = load_config(&state).ok();
    let mut manager = state.services.lock().map_err(|e| e.to_string())?;
    Ok(get_service_status_inner(&mut manager, config.as_ref()))
}

#[tauri::command]
pub async fn start_mail_gateway(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ServiceStatus, String> {
    let config = load_config(&state)?;
    let repo_root = repo_root_string()?;
    let spec = build_mail_gateway_spec(&config, &repo_root)?;
    let mut manager = state.services.lock().map_err(|e| e.to_string())?;
    let status = manager.start_mail_gateway(spec)?;
    emit_service_status(&app, crate::service_manager::MAIL_GATEWAY_SERVICE, &status);
    Ok(status)
}

#[tauri::command]
pub async fn stop_mail_gateway(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ServiceStatus, String> {
    let config = load_config(&state).ok();
    let probe_target = config
        .as_ref()
        .and_then(|items| build_mail_gateway_probe_target(items).ok());
    let mut manager = state.services.lock().map_err(|e| e.to_string())?;
    let status = manager.stop_mail_gateway(probe_target)?;
    emit_service_status(&app, crate::service_manager::MAIL_GATEWAY_SERVICE, &status);
    Ok(status)
}

#[tauri::command]
pub async fn start_turnstile_solver(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ServiceStatus, String> {
    let config = load_config(&state)?;
    let repo_root = repo_root_string()?;
    let spec = build_turnstile_solver_spec(&config, &repo_root)?;
    let mut manager = state.services.lock().map_err(|e| e.to_string())?;
    let status = manager.start_turnstile_solver(spec)?;
    emit_service_status(&app, crate::service_manager::TURNSTILE_SOLVER_SERVICE, &status);
    Ok(status)
}

#[tauri::command]
pub async fn stop_turnstile_solver(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ServiceStatus, String> {
    let config = load_config(&state).ok();
    let probe_target = config
        .as_ref()
        .and_then(|items| build_turnstile_solver_probe_target(items).ok());
    let mut manager = state.services.lock().map_err(|e| e.to_string())?;
    let status = manager.stop_turnstile_solver(probe_target)?;
    emit_service_status(
        &app,
        crate::service_manager::TURNSTILE_SOLVER_SERVICE,
        &status,
    );
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::get_service_status_inner;
    use crate::service_manager::{
        ServiceManager, MAIL_GATEWAY_SERVICE, TURNSTILE_SOLVER_SERVICE,
    };

    #[test]
    fn service_status_map_contains_known_services() {
        let mut manager = ServiceManager::default();
        let statuses = get_service_status_inner(&mut manager, None);

        assert!(statuses.contains_key(MAIL_GATEWAY_SERVICE));
        assert!(statuses.contains_key(TURNSTILE_SOLVER_SERVICE));
        assert!(!statuses[MAIL_GATEWAY_SERVICE].running);
        assert!(!statuses[TURNSTILE_SOLVER_SERVICE].running);
    }

    #[test]
    fn stopping_non_running_service_is_noop() {
        let mut manager = ServiceManager::default();

        let status = manager.stop_mail_gateway(None).expect("status");

        assert!(!status.running);
        assert!(status.pid.is_none());
        assert!(status.last_error.is_none());
    }
}
