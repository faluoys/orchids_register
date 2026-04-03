use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::State;

use crate::db;
use crate::orchids_profile;
use crate::state::AppState;

const ORCHIDS_API_DEFAULT_PROJECT_ID: &str = "280b7bae-cd29-41e4-a0a6-7f603c43b607";
const ORCHIDS_API_DEFAULT_AGENT_MODE: &str = "claude-opus-4.5";

#[derive(Debug, Serialize)]
struct OrchidsApiImportAccount {
    name: String,
    session_id: String,
    client_cookie: String,
    client_uat: String,
    project_id: String,
    user_id: String,
    agent_mode: String,
    email: String,
    weight: i32,
    enabled: bool,
}

#[derive(Debug, Serialize)]
struct OrchidsApiImportPayload {
    version: i32,
    export_at: String,
    accounts: Vec<OrchidsApiImportAccount>,
}

fn normalize_client_cookie(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed
        .trim_matches(|c| matches!(c, '"' | '[' | ']'))
        .trim()
        .to_string();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn normalize_optional_text(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn build_profile_session_context(account: &db::Account) -> Option<orchids_profile::ExistingSessionContext> {
    let session_id = normalize_optional_text(account.created_session_id.as_deref());
    let user_id = normalize_optional_text(account.created_user_id.as_deref());
    let session_jwt = normalize_optional_text(account.desktop_jwt.as_deref());

    let has_way_to_get_jwt = session_jwt.is_some() || session_id.is_some();
    let has_way_to_get_user = user_id.is_some() || session_id.is_some();
    if has_way_to_get_jwt && has_way_to_get_user {
        Some(orchids_profile::ExistingSessionContext {
            session_id,
            user_id,
            session_jwt,
        })
    } else {
        None
    }
}

fn filter_accounts_by_ids(
    accounts: Vec<db::Account>,
    ids: Option<&[i64]>,
) -> Vec<db::Account> {
    let Some(ids) = ids else {
        return accounts;
    };

    let id_to_index: HashMap<i64, usize> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();

    let mut selected: Vec<(usize, db::Account)> = accounts
        .into_iter()
        .filter_map(|account| id_to_index.get(&account.id).copied().map(|index| (index, account)))
        .collect();
    selected.sort_by_key(|(index, _)| *index);
    selected.into_iter().map(|(_, account)| account).collect()
}

fn build_orchids_api_rows(accounts: &[db::Account]) -> Vec<OrchidsApiImportAccount> {
    let client_uat = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string());

    accounts
        .iter()
        .filter_map(|account| {
            let session_id = account.created_session_id.as_deref()?.trim();
            let user_id = account.created_user_id.as_deref()?.trim();
            let client_cookie = normalize_client_cookie(account.client_cookie.as_deref()?)?;

            if session_id.is_empty() || user_id.is_empty() || client_cookie.is_empty() {
                return None;
            }

            Some(OrchidsApiImportAccount {
                name: format!("orchids-{}", account.id),
                session_id: session_id.to_string(),
                client_cookie,
                client_uat: client_uat.clone(),
                project_id: ORCHIDS_API_DEFAULT_PROJECT_ID.to_string(),
                user_id: user_id.to_string(),
                agent_mode: ORCHIDS_API_DEFAULT_AGENT_MODE.to_string(),
                email: account.email.clone(),
                weight: 1,
                enabled: true,
            })
        })
        .collect()
}

fn render_export_payload(accounts: &[db::Account], format: &str) -> Result<String, String> {
    match format {
        "csv" => {
            let mut wtr = csv::Writer::from_writer(Vec::new());
            wtr.write_record([
                "ID",
                "Email",
                "Password",
                "Status",
                "Group",
                "Plan",
                "Credits",
                "SignUpId",
                "ClientCookie",
                "DesktopJWT",
                "CreatedAt",
            ])
            .map_err(|e| e.to_string())?;
            for account in accounts {
                let credits = account.credits.map(|v| v.to_string()).unwrap_or_default();
                wtr.write_record([
                    &account.id.to_string(),
                    &account.email,
                    &account.password,
                    &account.status,
                    &account.group_name,
                    account.plan.as_deref().unwrap_or(""),
                    &credits,
                    account.sign_up_id.as_deref().unwrap_or(""),
                    account.client_cookie.as_deref().unwrap_or(""),
                    account.desktop_jwt.as_deref().unwrap_or(""),
                    &account.created_at,
                ])
                .map_err(|e| e.to_string())?;
            }
            let data = wtr.into_inner().map_err(|e| e.to_string())?;
            String::from_utf8(data).map_err(|e| e.to_string())
        }
        "cookie" => {
            let rows: Vec<String> = accounts
                .iter()
                .filter_map(|account| account.client_cookie.as_deref())
                .filter_map(normalize_client_cookie)
                .collect();
            serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())
        }
        "orchids-api" => {
            let payload = OrchidsApiImportPayload {
                version: 1,
                export_at: chrono::Utc::now().to_rfc3339(),
                accounts: build_orchids_api_rows(accounts),
            };
            serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
        }
        _ => serde_json::to_string_pretty(accounts).map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub async fn get_accounts(
    state: State<'_, AppState>,
    status: Option<String>,
    group_id: Option<i64>,
) -> Result<Vec<db::Account>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_all_accounts(&conn, status.as_deref(), group_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_accounts_profile_missing(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<usize, String> {
    let to_refresh = limit.unwrap_or(5).clamp(1, 10);

    // 1) 读出需要补全的账号列表（快速释放锁）
    let candidates: Vec<(i64, String, String, Option<orchids_profile::ExistingSessionContext>)> = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let all = db::get_all_accounts(&conn, None, None).map_err(|e| e.to_string())?;
        all.into_iter()
            .filter(|a| {
                let can_reuse_session = build_profile_session_context(a).is_some();
                (a.register_complete || a.status == "complete")
                    && (a.plan.is_none() || a.credits.is_none())
                    && (can_reuse_session || !a.password.trim().is_empty())
            })
            .take(to_refresh)
            .map(|a| {
                let existing_session = build_profile_session_context(&a);
                (a.id, a.email, a.password, existing_session)
            })
            .collect()
    };

    if candidates.is_empty() {
        return Ok(0);
    }

    // 从 DB 读取 proxy 配置
    let proxy: Option<String> = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::get_all_config(&conn)
            .ok()
            .and_then(|c| c.get("proxy").cloned())
            .filter(|p| !p.is_empty())
    };

    // 2) 为每个账号启动独立的 spawn_blocking 任务，实现并行 HTTP 调用
    let db: Arc<_> = state.db.clone();
    let mut handles = Vec::with_capacity(candidates.len());

    for (id, email, password, existing_session) in candidates {
        let db = Arc::clone(&db);
        let proxy = proxy.clone();
        let handle = tokio::task::spawn_blocking(move || {
            match orchids_profile::fetch_plan_and_credits_with_session(
                &email,
                &password,
                30,
                proxy.as_deref(),
                existing_session.as_ref(),
            ) {
                Ok(profile) => {
                    if let Ok(conn) = db.lock() {
                        let _ = db::update_account_plan_credits(
                            &conn,
                            id,
                            profile.plan.as_deref(),
                            profile.credits,
                        );
                    }
                    eprintln!(
                        "[profile] 补全成功: {} -> plan={:?}, credits={:?}",
                        email, profile.plan, profile.credits
                    );
                    true
                }
                Err(err) => {
                    eprintln!("[profile] 补全失败: {} -> {}", email, err);
                    false
                }
            }
        });
        handles.push(handle);
    }

    // 3) 等待所有并行任务完成，统计成功数
    let results = futures::future::join_all(handles).await;
    let refreshed = results
        .into_iter()
        .filter(|r| matches!(r, Ok(true)))
        .count();

    Ok(refreshed)
}

#[tauri::command]
pub async fn refresh_account_profile(
    state: State<'_, AppState>,
    id: i64,
) -> Result<db::Account, String> {
    let (email, password, proxy, existing_session) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let all = db::get_all_accounts(&conn, None, None).map_err(|e| e.to_string())?;
        let account = all
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| "账号不存在".to_string())?;
        let existing_session = build_profile_session_context(&account);
        let proxy = db::get_all_config(&conn)
            .ok()
            .and_then(|c| c.get("proxy").cloned())
            .filter(|p| !p.is_empty());
        (account.email, account.password, proxy, existing_session)
    };

    if password.trim().is_empty() && existing_session.is_none() {
        return Err("该账号密码为空，无法刷新套餐和 credits".to_string());
    }

    let profile = tokio::task::spawn_blocking(move || {
        orchids_profile::fetch_plan_and_credits_with_session(
            &email,
            &password,
            30,
            proxy.as_deref(),
            existing_session.as_ref(),
        )
    })
    .await
    .map_err(|e| format!("刷新任务执行失败: {}", e))?
    .map_err(|e| format!("刷新失败: {}", e))?;

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::update_account_plan_credits(&conn, id, profile.plan.as_deref(), profile.credits)
        .map_err(|e| e.to_string())?;

    let all = db::get_all_accounts(&conn, None, None).map_err(|e| e.to_string())?;
    all.into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| "刷新后未找到账号记录".to_string())
}

#[tauri::command]
pub async fn delete_account(
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_account_by_id(&conn, id).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_accounts(
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> Result<usize, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_accounts_by_ids(&conn, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_accounts(
    state: State<'_, AppState>,
    status: Option<String>,
    format: Option<String>,
    ids: Option<Vec<i64>>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let accounts = db::get_all_accounts(&conn, status.as_deref(), None).map_err(|e| e.to_string())?;
    let selected = filter_accounts_by_ids(accounts, ids.as_deref());
    let fmt = format.unwrap_or_else(|| "json".to_string());
    render_export_payload(&selected, &fmt)
}

#[tauri::command]
pub async fn list_account_groups(state: State<'_, AppState>) -> Result<Vec<db::AccountGroup>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::list_account_groups(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_account_group(
    state: State<'_, AppState>,
    name: String,
) -> Result<db::AccountGroup, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("分组名称不能为空".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let id = db::create_account_group(&conn, &name).map_err(|e| e.to_string())?;
    db::get_account_group_by_id(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "创建分组失败".to_string())
}

#[tauri::command]
pub async fn rename_account_group(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("分组名称不能为空".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let group = db::get_account_group_by_id(&conn, id).map_err(|e| e.to_string())?;
    match group {
        Some(g) if g.is_default => Err("默认分组不支持重命名".to_string()),
        Some(_) => db::rename_account_group(&conn, id, &name).map_err(|e| e.to_string()),
        None => Err("分组不存在".to_string()),
    }
}

#[tauri::command]
pub async fn delete_account_group(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let group = db::get_account_group_by_id(&conn, id).map_err(|e| e.to_string())?;
    match group {
        Some(g) if g.is_default => Err("默认分组不支持删除".to_string()),
        Some(_) => db::delete_account_group(&conn, id).map_err(|e| e.to_string()),
        None => Err("分组不存在".to_string()),
    }
}

#[tauri::command]
pub async fn set_account_group_pinned(
    state: State<'_, AppState>,
    id: i64,
    pinned: bool,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let group = db::get_account_group_by_id(&conn, id).map_err(|e| e.to_string())?;
    match group {
        Some(g) if g.is_default && !pinned => Err("默认分组必须置顶".to_string()),
        Some(_) => db::set_account_group_pinned(&conn, id, pinned).map_err(|e| e.to_string()),
        None => Err("分组不存在".to_string()),
    }
}

#[tauri::command]
pub async fn move_account_group(
    state: State<'_, AppState>,
    id: i64,
    direction: String,
) -> Result<(), String> {
    if direction != "up" && direction != "down" {
        return Err("无效的移动方向".to_string());
    }

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let group = db::get_account_group_by_id(&conn, id).map_err(|e| e.to_string())?;
    match group {
        Some(g) if g.is_default => Ok(()),
        Some(_) => db::move_account_group(&conn, id, &direction).map_err(|e| e.to_string()),
        None => Err("分组不存在".to_string()),
    }
}

#[tauri::command]
pub async fn move_accounts_to_group(
    state: State<'_, AppState>,
    ids: Vec<i64>,
    target_group_id: i64,
) -> Result<usize, String> {
    if ids.is_empty() {
        return Ok(0);
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let target = db::get_account_group_by_id(&conn, target_group_id).map_err(|e| e.to_string())?;
    if target.is_none() {
        return Err("目标分组不存在".to_string());
    }
    db::move_accounts_to_group(&conn, &ids, target_group_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_text_file(
    app: tauri::AppHandle,
    content: String,
    default_name: String,
) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = tokio::task::spawn_blocking(move || {
        let file_path = app
            .dialog()
            .file()
            .set_file_name(&default_name)
            .add_filter("JSON", &["json"])
            .add_filter("Text", &["txt", "jsonl"])
            .add_filter("所有文件", &["*"])
            .blocking_save_file();

        match file_path {
            Some(path) => {
                let p = path.as_path().ok_or_else(|| "无法获取文件路径".to_string())?;
                std::fs::write(p, &content).map_err(|e| format!("写入文件失败: {}", e))?;
                Ok(true)
            }
            None => Ok(false),
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn sample_account() -> db::Account {
        db::Account {
            id: 7,
            email: "demo@example.com".to_string(),
            password: "Secret123!".to_string(),
            sign_up_id: Some("sua_123".to_string()),
            email_code: None,
            register_complete: true,
            created_session_id: Some("sess_123".to_string()),
            created_user_id: Some("user_123".to_string()),
            client_cookie: Some("\"cookie_123\"".to_string()),
            desktop_jwt: Some("jwt_123".to_string()),
            status: "complete".to_string(),
            error_message: None,
            batch_id: None,
            plan: Some("FREE".to_string()),
            credits: Some(12),
            created_at: "2026-04-03 16:00:00".to_string(),
            updated_at: "2026-04-03 16:00:00".to_string(),
            group_id: 1,
            group_name: "默认分组".to_string(),
        }
    }

    #[test]
    fn orchids_api_export_uses_expected_shape() {
        let payload = render_export_payload(&[sample_account()], "orchids-api").unwrap();
        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value.get("version").and_then(Value::as_i64), Some(1));
        assert!(value.get("export_at").and_then(Value::as_str).is_some());

        let rows = value.get("accounts").and_then(Value::as_array).unwrap();
        assert_eq!(rows.len(), 1);
        let row = rows[0].as_object().unwrap();
        assert_eq!(row.get("name").and_then(Value::as_str), Some("orchids-7"));
        assert_eq!(row.get("session_id").and_then(Value::as_str), Some("sess_123"));
        assert_eq!(row.get("client_cookie").and_then(Value::as_str), Some("cookie_123"));
        assert_eq!(
            row.get("project_id").and_then(Value::as_str),
            Some("280b7bae-cd29-41e4-a0a6-7f603c43b607")
        );
        assert_eq!(row.get("user_id").and_then(Value::as_str), Some("user_123"));
        assert_eq!(row.get("agent_mode").and_then(Value::as_str), Some("claude-opus-4.5"));
        assert_eq!(row.get("email").and_then(Value::as_str), Some("demo@example.com"));
        assert_eq!(row.get("weight").and_then(Value::as_i64), Some(1));
        assert_eq!(row.get("enabled").and_then(Value::as_bool), Some(true));
        assert!(row.get("client_uat").and_then(Value::as_str).is_some());
    }

    #[test]
    fn orchids_api_export_skips_accounts_missing_required_fields() {
        let mut missing = sample_account();
        missing.created_session_id = None;

        let payload = render_export_payload(&[missing], "orchids-api").unwrap();
        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value.get("accounts").and_then(Value::as_array).unwrap().len(), 0);
    }

    #[test]
    fn cookie_export_returns_sanitized_cookie_array() {
        let payload = render_export_payload(&[sample_account()], "cookie").unwrap();
        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value, serde_json::json!(["cookie_123"]));
    }

    #[test]
    fn build_profile_session_context_uses_saved_jwt_when_available() {
        let context = build_profile_session_context(&sample_account()).expect("应构建出复用会话");

        assert_eq!(context.session_id.as_deref(), Some("sess_123"));
        assert_eq!(context.user_id.as_deref(), Some("user_123"));
        assert_eq!(context.session_jwt.as_deref(), Some("jwt_123"));
    }

    #[test]
    fn build_profile_session_context_accepts_session_id_without_jwt() {
        let mut account = sample_account();
        account.desktop_jwt = None;

        let context = build_profile_session_context(&account).expect("session_id 应可用于后续换 jwt");

        assert_eq!(context.session_id.as_deref(), Some("sess_123"));
        assert_eq!(context.user_id.as_deref(), Some("user_123"));
        assert_eq!(context.session_jwt, None);
    }

    #[test]
    fn build_profile_session_context_rejects_incomplete_saved_session() {
        let mut account = sample_account();
        account.created_session_id = None;
        account.created_user_id = None;

        let context = build_profile_session_context(&account);

        assert!(context.is_none(), "缺少 user_id/session_id 时不应尝试复用会话");
    }
}
