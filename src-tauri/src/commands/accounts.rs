use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::blocking::ClientBuilder;
use serde::Serialize;
use tauri::{Manager, State, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::db;
use crate::orchids_profile;
use crate::state::AppState;

const ORCHIDS_API_DEFAULT_PROJECT_ID: &str = "280b7bae-cd29-41e4-a0a6-7f603c43b607";
const ORCHIDS_API_DEFAULT_AGENT_MODE: &str = "claude-opus-4.5";
const EXPORT_DESKTOP_JWT_TIMEOUT_SECS: i64 = 15;
const JWT_REFRESH_BUFFER_SECS: i64 = 5 * 60;
const DESKTOP_CLERK_JS_VERSION: &str = "5.117.0";
const DESKTOP_CLERK_API_VERSION: &str = "2025-11-10";
const DESKTOP_SESSION_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Orchids/1.0.6 Chrome/138.0.7204.251 Electron/37.10.3 Safari/537.36";

#[derive(Debug, Serialize)]
struct OrchidsApiImportAccount {
    name: String,
    session_id: String,
    client_cookie: String,
    client_uat: String,
    project_id: String,
    user_id: String,
    desktop_jwt: String,
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

struct CompletionCheckInput {
    email: String,
    password: String,
    proxy: Option<String>,
    existing_session: Option<orchids_profile::ExistingSessionContext>,
}

fn find_account_by_id(conn: &rusqlite::Connection, id: i64) -> Result<db::Account, String> {
    db::get_all_accounts(conn, None, None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|account| account.id == id)
        .ok_or_else(|| "账号不存在".to_string())
}

fn load_completion_check_input(
    state: &AppState,
    id: i64,
) -> Result<CompletionCheckInput, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let account = find_account_by_id(&conn, id)?;
    let existing_session = build_profile_session_context(&account);
    let proxy = db::get_all_config(&conn)
        .ok()
        .and_then(|config| config.get("proxy").cloned())
        .filter(|value| !value.is_empty());

    if account.password.trim().is_empty() && existing_session.is_none() {
        return Err("该账号缺少密码，且没有可复用会话，无法继续检测补全状态".to_string());
    }

    Ok(CompletionCheckInput {
        email: account.email,
        password: account.password,
        proxy,
        existing_session,
    })
}

#[cfg(test)]
fn check_account_completion_with_fetcher<F>(
    state: &AppState,
    id: i64,
    fetcher: F,
) -> Result<db::Account, String>
where
    F: FnOnce(
        &str,
        &str,
        i64,
        Option<&str>,
        Option<&orchids_profile::ExistingSessionContext>,
    ) -> Result<orchids_profile::ProfileResult, String>,
{
    let input = load_completion_check_input(state, id)?;
    let profile = fetcher(
        &input.email,
        &input.password,
        30,
        input.proxy.as_deref(),
        input.existing_session.as_ref(),
    )?;

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::update_account_plan_credits(&conn, id, profile.plan.as_deref(), profile.credits)
        .map_err(|e| e.to_string())?;
    find_account_by_id(&conn, id)
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
    let fallback_client_uat = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string());

    accounts
        .iter()
        .filter_map(|account| {
            let session_id = account.created_session_id.as_deref()?.trim();
            let user_id = account.created_user_id.as_deref()?.trim();
            let client_cookie = normalize_client_cookie(account.client_cookie.as_deref()?)?;
            let client_uat = normalize_optional_text(account.client_uat.as_deref())
                .unwrap_or_else(|| fallback_client_uat.clone());

            if session_id.is_empty() || user_id.is_empty() || client_cookie.is_empty() {
                return None;
            }

            Some(OrchidsApiImportAccount {
                name: format!("orchids-{}", account.id),
                session_id: session_id.to_string(),
                client_cookie,
                client_uat,
                project_id: ORCHIDS_API_DEFAULT_PROJECT_ID.to_string(),
                user_id: user_id.to_string(),
                desktop_jwt: account.desktop_jwt.clone().unwrap_or_default(),
                agent_mode: ORCHIDS_API_DEFAULT_AGENT_MODE.to_string(),
                email: account.email.clone(),
                weight: 1,
                enabled: true,
            })
        })
        .collect()
}

fn apply_refreshed_desktop_jwts(
    accounts: &mut [db::Account],
    refreshed_by_id: &HashMap<i64, String>,
) {
    for account in accounts {
        if let Some(jwt) = refreshed_by_id.get(&account.id) {
            account.desktop_jwt = Some(jwt.clone());
        }
    }
}

fn refresh_desktop_jwts_for_export(
    accounts: &[db::Account],
    proxy: Option<&str>,
) -> HashMap<i64, String> {
    let mut refreshed = HashMap::new();

    for account in accounts {
        if let Some(jwt) = maybe_refresh_desktop_jwt_for_export(account, proxy) {
            refreshed.insert(account.id, jwt);
        }
    }

    refreshed
}

fn maybe_refresh_desktop_jwt_for_export(
    account: &db::Account,
    proxy: Option<&str>,
) -> Option<String> {
    if !desktop_jwt_needs_refresh(account.desktop_jwt.as_deref(), current_unix_timestamp()) {
        return None;
    }

    let session_id = normalize_optional_text(account.created_session_id.as_deref())?;
    let client_cookie = normalize_client_cookie(account.client_cookie.as_deref()?)?;
    let client_uat = effective_client_uat(account);

    match fetch_fresh_desktop_jwt(
        &session_id,
        &client_cookie,
        &client_uat,
        proxy,
        EXPORT_DESKTOP_JWT_TIMEOUT_SECS,
    ) {
        Ok(jwt) => Some(jwt),
        Err(err) => {
            eprintln!(
                "[export] 刷新 desktop_jwt 失败: account_id={}, email={} -> {}",
                account.id, account.email, err
            );
            None
        }
    }
}

fn fetch_fresh_desktop_jwt(
    session_id: &str,
    client_cookie: &str,
    client_uat: &str,
    proxy: Option<&str>,
    timeout_secs: i64,
) -> Result<String, String> {
    let mut builder = ClientBuilder::new();
    if let Some(proxy_url) = proxy.filter(|value| !value.trim().is_empty()) {
        let configured_proxy = reqwest::Proxy::all(proxy_url)
            .map_err(|e| format!("无效的代理地址: {}", e))?
            .no_proxy(reqwest::NoProxy::from_string("localhost,127.0.0.1"));
        builder = builder.proxy(configured_proxy);
    }

    let client = builder
        .build()
        .map_err(|e| format!("创建导出客户端失败: {}", e))?;

    let url = format!("https://clerk.orchids.app/v1/client/sessions/{session_id}/tokens");
    let response = client
        .post(url)
        .query(&[
            ("__clerk_api_version", DESKTOP_CLERK_API_VERSION),
            ("_clerk_js_version", DESKTOP_CLERK_JS_VERSION),
            ("debug", "skip_cache"),
        ])
        .header("accept", "*/*")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("user-agent", DESKTOP_SESSION_USER_AGENT)
        .header("accept-language", "zh-CN,zh;q=0.9")
        .header(
            "cookie",
            format!("__client={client_cookie}; __client_uat={client_uat}"),
        )
        .form(&[("organization_id", "")])
        .timeout(std::time::Duration::from_secs(timeout_secs.max(0) as u64))
        .send()
        .map_err(|e| format!("获取 session jwt 失败: {}", e))?;

    let status = response.status().as_u16();
    let body = response.text().unwrap_or_default();
    if status >= 400 {
        return Err(format!("获取 session jwt 失败: HTTP {} -> {}", status, body));
    }

    let value: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|_| serde_json::json!({ "raw": body }));
    find_first_jwt(&value)
        .ok_or_else(|| format!("tokens 响应里未找到 jwt: {}", value))
}

fn find_first_jwt(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(jwt) = map
                .get("jwt")
                .and_then(serde_json::Value::as_str)
                .filter(|current| !current.is_empty())
            {
                return Some(jwt.to_string());
            }

            for child in map.values() {
                if let Some(jwt) = find_first_jwt(child) {
                    return Some(jwt);
                }
            }

            None
        }
        serde_json::Value::Array(items) => {
            for child in items {
                if let Some(jwt) = find_first_jwt(child) {
                    return Some(jwt);
                }
            }
            None
        }
        _ => None,
    }
}

fn current_unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn effective_client_uat(account: &db::Account) -> String {
    normalize_optional_text(account.client_uat.as_deref())
        .unwrap_or_else(|| current_unix_timestamp().to_string())
}

fn desktop_jwt_needs_refresh(jwt: Option<&str>, now_unix: i64) -> bool {
    match jwt.and_then(jwt_exp_unix) {
        Some(exp) => exp <= now_unix + JWT_REFRESH_BUFFER_SECS,
        None => true,
    }
}

fn jwt_exp_unix(jwt: &str) -> Option<i64> {
    let payload = jwt.split('.').nth(1)?;
    let decoded = decode_base64_url(payload)?;
    let value: serde_json::Value = serde_json::from_slice(&decoded).ok()?;

    value
        .get("exp")
        .and_then(serde_json::Value::as_i64)
        .or_else(|| {
            value
                .get("exp")
                .and_then(serde_json::Value::as_u64)
                .and_then(|current| i64::try_from(current).ok())
        })
}

fn decode_base64_url(input: &str) -> Option<Vec<u8>> {
    let mut buffer = Vec::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' => buffer.push(byte),
            _ => return None,
        }
    }

    while buffer.len() % 4 != 0 {
        buffer.push(b'=');
    }

    let mut output = Vec::with_capacity(buffer.len() / 4 * 3);
    for chunk in buffer.chunks(4) {
        let a = decode_base64_url_char(chunk[0])?;
        let b = decode_base64_url_char(chunk[1])?;
        let c = if chunk[2] == b'=' {
            64
        } else {
            decode_base64_url_char(chunk[2])?
        };
        let d = if chunk[3] == b'=' {
            64
        } else {
            decode_base64_url_char(chunk[3])?
        };

        output.push((a << 2) | (b >> 4));
        if c != 64 {
            output.push(((b & 0x0f) << 4) | (c >> 2));
        }
        if d != 64 {
            output.push(((c & 0x03) << 6) | d);
        }
    }

    Some(output)
}

fn decode_base64_url_char(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        b'=' => Some(64),
        _ => None,
    }
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
pub async fn check_account_completion(
    state: State<'_, AppState>,
    account_id: i64,
) -> Result<db::Account, String> {
    let input = load_completion_check_input(state.inner(), account_id)?;
    let profile = tokio::task::spawn_blocking(move || {
        orchids_profile::fetch_plan_and_credits_with_session(
            &input.email,
            &input.password,
            30,
            input.proxy.as_deref(),
            input.existing_session.as_ref(),
        )
    })
    .await
    .map_err(|e| format!("补全检测任务执行失败: {}", e))?
    .map_err(|e| format!("补全检测失败: {}", e))?;

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::update_account_plan_credits(
        &conn,
        account_id,
        profile.plan.as_deref(),
        profile.credits,
    )
    .map_err(|e| e.to_string())?;
    find_account_by_id(&conn, account_id)
}

#[tauri::command]
pub async fn open_account_completion_window(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    account_id: i64,
) -> Result<(), String> {
    let account = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        find_account_by_id(&conn, account_id)?
    };
    let label = format!("account-completion-{}", account_id);

    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    if state.has_completion_window(account_id)? {
        state.clear_completion_window(account_id)?;
    }
    state.try_register_completion_window(account_id)?;

    let target_url = Url::parse("https://www.orchids.app/")
        .map_err(|e| format!("补全窗口 URL 无效: {}", e))?;

    let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(target_url))
        .title(format!("继续补全: {}", account.email))
        .inner_size(1280.0, 900.0)
        .min_inner_size(960.0, 720.0)
        .resizable(true)
        .build()
        .map_err(|e| {
            let _ = state.clear_completion_window(account_id);
            format!("打开补全窗口失败: {}", e)
        })?;

    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            let state = app_handle.state::<AppState>();
            let _ = state.clear_completion_window(account_id);
        }
    });

    Ok(())
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
    let fmt = format.unwrap_or_else(|| "json".to_string());
    let (mut selected, proxy) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let accounts =
            db::get_all_accounts(&conn, status.as_deref(), None).map_err(|e| e.to_string())?;
        let selected = filter_accounts_by_ids(accounts, ids.as_deref());
        let proxy = db::get_all_config(&conn)
            .ok()
            .and_then(|config| config.get("proxy").cloned())
            .filter(|value| !value.is_empty());
        (selected, proxy)
    };

    if fmt == "orchids-api" {
        let selected_for_refresh = selected.clone();
        let refreshed = tokio::task::spawn_blocking(move || {
            refresh_desktop_jwts_for_export(&selected_for_refresh, proxy.as_deref())
        })
        .await
        .map_err(|e| format!("导出前刷新 desktop_jwt 任务失败: {}", e))?;
        apply_refreshed_desktop_jwts(&mut selected, &refreshed);
    }

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
            client_uat: Some("uat_123".to_string()),
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
        assert_eq!(row.get("client_uat").and_then(Value::as_str), Some("uat_123"));
        assert_eq!(
            row.get("project_id").and_then(Value::as_str),
            Some("280b7bae-cd29-41e4-a0a6-7f603c43b607")
        );
        assert_eq!(row.get("user_id").and_then(Value::as_str), Some("user_123"));
        assert_eq!(row.get("desktop_jwt").and_then(Value::as_str), Some("jwt_123"));
        assert_eq!(row.get("agent_mode").and_then(Value::as_str), Some("claude-opus-4.5"));
        assert_eq!(row.get("email").and_then(Value::as_str), Some("demo@example.com"));
        assert_eq!(row.get("weight").and_then(Value::as_i64), Some(1));
        assert_eq!(row.get("enabled").and_then(Value::as_bool), Some(true));
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
    fn orchids_api_export_falls_back_when_client_uat_missing() {
        let mut account = sample_account();
        account.client_uat = None;

        let payload = render_export_payload(&[account], "orchids-api").unwrap();
        let value: Value = serde_json::from_str(&payload).unwrap();
        let rows = value.get("accounts").and_then(Value::as_array).unwrap();
        let row = rows[0].as_object().unwrap();

        assert_eq!(row.get("desktop_jwt").and_then(Value::as_str), Some("jwt_123"));
        assert!(row.get("client_uat").and_then(Value::as_str).is_some());
    }

    #[test]
    fn desktop_jwt_needs_refresh_when_expired() {
        let expired = "eyJhbGciOiJub25lIn0.eyJleHAiOjE3MDAwMDAwMDB9.sig";
        let still_valid = "eyJhbGciOiJub25lIn0.eyJleHAiOjQxMDI0NDQ4MDB9.sig";

        assert!(desktop_jwt_needs_refresh(Some(expired), 1_775_243_784));
        assert!(!desktop_jwt_needs_refresh(Some(still_valid), 1_775_243_784));
        assert!(desktop_jwt_needs_refresh(None, 1_775_243_784));
    }

    #[test]
    fn refreshed_desktop_jwt_overrides_stale_value_for_export() {
        let mut accounts = vec![sample_account()];
        let refreshed = HashMap::from([(7_i64, "jwt_fresh".to_string())]);

        apply_refreshed_desktop_jwts(&mut accounts, &refreshed);

        let payload = render_export_payload(&accounts, "orchids-api").unwrap();
        let value: Value = serde_json::from_str(&payload).unwrap();
        let rows = value.get("accounts").and_then(Value::as_array).unwrap();
        let row = rows[0].as_object().unwrap();

        assert_eq!(row.get("desktop_jwt").and_then(Value::as_str), Some("jwt_fresh"));
    }

    #[test]
    fn effective_client_uat_falls_back_to_current_timestamp_when_missing() {
        let mut account = sample_account();
        account.client_uat = None;

        let client_uat = effective_client_uat(&account);
        let parsed = client_uat.parse::<i64>().expect("fallback client_uat 应为时间戳");

        assert!(parsed > 0);
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
        assert!(context.is_none(), "missing user_id/session_id should disable saved session reuse");
    }
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    use crate::state::AppState;

    fn insert_completion_candidate(state: &AppState, email: &str) -> i64 {
        let conn = state.db.lock().expect("db lock should succeed");
        let account_id = db::insert_account(
            &conn,
            &db::NewAccount {
                email: email.to_string(),
                password: "Secret123!".to_string(),
                status: "complete".to_string(),
                batch_id: None,
                group_id: None,
            },
        )
        .expect("account should insert");

        db::update_account_result(
            &conn,
            account_id,
            email,
            Some("sua_completion"),
            None,
            true,
            Some("sess_completion"),
            Some("user_completion"),
            Some("cookie_completion"),
            Some("uat_completion"),
            Some("jwt_completion"),
            "complete",
            None,
        )
        .expect("account result should update");

        account_id
    }

    #[test]
    fn completion_check_updates_account_on_success() {
        let state = AppState::new().expect("state should initialize");
        let account_id = insert_completion_candidate(&state, "completion-success@example.com");

        let refreshed = check_account_completion_with_fetcher(
            &state,
            account_id,
            |email, password, timeout, _proxy, existing_session| {
                assert_eq!(email, "completion-success@example.com");
                assert_eq!(password, "Secret123!");
                assert_eq!(timeout, 30);
                assert_eq!(
                    existing_session,
                    Some(&orchids_profile::ExistingSessionContext {
                        session_id: Some("sess_completion".to_string()),
                        user_id: Some("user_completion".to_string()),
                        session_jwt: Some("jwt_completion".to_string()),
                    })
                );

                Ok(orchids_profile::ProfileResult {
                    plan: Some("PRO".to_string()),
                    credits: Some(88),
                })
            },
        )
        .expect("completion check should succeed");

        assert_eq!(refreshed.id, account_id);
        assert_eq!(refreshed.plan.as_deref(), Some("PRO"));
        assert_eq!(refreshed.credits, Some(88));
    }

    #[test]
    fn completion_check_keeps_account_unchanged_on_failure() {
        let state = AppState::new().expect("state should initialize");
        let account_id = insert_completion_candidate(&state, "completion-failed@example.com");

        let error = check_account_completion_with_fetcher(
            &state,
            account_id,
            |_email, _password, _timeout, _proxy, _existing_session| {
                Err("visitor verification still pending".to_string())
            },
        )
        .expect_err("completion check should fail");

        assert!(error.contains("visitor verification still pending"));

        let conn = state.db.lock().expect("db lock should succeed");
        let account = db::get_all_accounts(&conn, None, None)
            .expect("accounts should load")
            .into_iter()
            .find(|account| account.id == account_id)
            .expect("account should exist");

        assert_eq!(account.plan, None);
        assert_eq!(account.credits, None);
    }
}
