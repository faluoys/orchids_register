use std::sync::Arc;

use tauri::State;

use crate::db;
use crate::orchids_profile;
use crate::state::AppState;

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
    let candidates: Vec<(i64, String, String)> = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let all = db::get_all_accounts(&conn, None, None).map_err(|e| e.to_string())?;
        all.into_iter()
            .filter(|a| {
                (a.register_complete || a.status == "complete")
                    && (a.plan.is_none() || a.credits.is_none())
                    && !a.password.trim().is_empty()
            })
            .take(to_refresh)
            .map(|a| (a.id, a.email, a.password))
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

    for (id, email, password) in candidates {
        let db = Arc::clone(&db);
        let proxy = proxy.clone();
        let handle = tokio::task::spawn_blocking(move || {
            match orchids_profile::fetch_plan_and_credits(&email, &password, 30, proxy.as_deref()) {
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
    let (email, password, proxy) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let all = db::get_all_accounts(&conn, None, None).map_err(|e| e.to_string())?;
        let account = all
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| "账号不存在".to_string())?;
        let proxy = db::get_all_config(&conn)
            .ok()
            .and_then(|c| c.get("proxy").cloned())
            .filter(|p| !p.is_empty());
        (account.email, account.password, proxy)
    };

    if password.trim().is_empty() {
        return Err("该账号密码为空，无法刷新套餐和 credits".to_string());
    }

    let profile = tokio::task::spawn_blocking(move || {
        orchids_profile::fetch_plan_and_credits(&email, &password, 30, proxy.as_deref())
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
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let accounts = db::get_all_accounts(&conn, status.as_deref(), None).map_err(|e| e.to_string())?;

    let fmt = format.unwrap_or_else(|| "json".to_string());
    match fmt.as_str() {
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
            for a in &accounts {
                let credits = a.credits.map(|v| v.to_string()).unwrap_or_default();
                wtr.write_record([
                    &a.id.to_string(),
                    &a.email,
                    &a.password,
                    &a.status,
                    &a.group_name,
                    a.plan.as_deref().unwrap_or(""),
                    &credits,
                    a.sign_up_id.as_deref().unwrap_or(""),
                    a.client_cookie.as_deref().unwrap_or(""),
                    a.desktop_jwt.as_deref().unwrap_or(""),
                    &a.created_at,
                ])
                .map_err(|e| e.to_string())?;
            }
            let data = wtr.into_inner().map_err(|e| e.to_string())?;
            String::from_utf8(data).map_err(|e| e.to_string())
        }
        _ => serde_json::to_string_pretty(&accounts).map_err(|e| e.to_string()),
    }
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
            .add_filter("JSONL", &["jsonl"])
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
