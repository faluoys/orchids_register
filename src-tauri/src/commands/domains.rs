use tauri::State;

use crate::db;
use crate::state::AppState;

#[tauri::command]
pub async fn list_domains(state: State<'_, AppState>) -> Result<Vec<db::Domain>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::list_domains(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_domain(
    state: State<'_, AppState>,
    domain: String,
    enabled: bool,
) -> Result<db::Domain, String> {
    let domain = domain.trim().to_string();
    if domain.is_empty() {
        return Err("域名不能为空".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let id = db::create_domain(&conn, &domain, enabled).map_err(|e| e.to_string())?;
    db::sync_register_domain_id(&conn).map_err(|e| e.to_string())?;
    let created = db::get_domain_by_id(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "创建域名失败".to_string())?;
    Ok(created)
}

#[tauri::command]
pub async fn update_domain(
    state: State<'_, AppState>,
    id: i64,
    domain: String,
    enabled: bool,
) -> Result<(), String> {
    let domain = domain.trim().to_string();
    if domain.is_empty() {
        return Err("域名不能为空".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let existing = db::get_domain_by_id(&conn, id).map_err(|e| e.to_string())?;
    if existing.is_none() {
        return Err("域名不存在".to_string());
    }

    db::update_domain(&conn, id, &domain, enabled).map_err(|e| e.to_string())?;
    db::sync_register_domain_id(&conn).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_domain(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let existing = db::get_domain_by_id(&conn, id).map_err(|e| e.to_string())?;
    if existing.is_none() {
        return Err("域名不存在".to_string());
    }

    db::delete_domain(&conn, id).map_err(|e| e.to_string())?;
    db::sync_register_domain_id(&conn).map_err(|e| e.to_string())?;
    Ok(())
}
