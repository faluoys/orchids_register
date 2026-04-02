use std::collections::HashMap;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::db::{self, NewAccount};
use crate::orchids_profile;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterArgs {
    pub email: Option<String>,
    pub password: Option<String>,
    pub captcha_token: Option<String>,
    pub use_capmonster: bool,
    pub captcha_api_url: String,
    pub captcha_timeout: i64,
    pub captcha_poll_interval: f64,
    pub captcha_website_url: String,
    pub captcha_website_key: String,
    pub email_code: Option<String>,
    pub locale: String,
    pub timeout: i64,
    pub mail_mode: String,
    pub mail_gateway_base_url: Option<String>,
    pub mail_gateway_api_key: Option<String>,
    pub mail_provider: String,
    pub mail_provider_mode: String,
    pub mail_project_code: Option<String>,
    pub mail_domain: Option<String>,
    pub poll_timeout: i64,
    pub poll_interval: f64,
    pub code_pattern: String,
    pub debug_email: bool,
    pub test_desktop_session: bool,
    pub proxy: Option<String>,
    pub use_proxy_pool: bool,
    pub proxy_pool_api: String,
}

impl RegisterArgs {
    pub fn to_cli_args(&self) -> orchids_core::cli::Args {
        orchids_core::cli::Args {
            email: self.email.clone(),
            password: self.password.clone(),
            captcha_token: self.captcha_token.clone(),
            use_capmonster: self.use_capmonster,
            captcha_api_url: self.captcha_api_url.clone(),
            captcha_timeout: self.captcha_timeout,
            captcha_poll_interval: self.captcha_poll_interval,
            captcha_website_url: self.captcha_website_url.clone(),
            captcha_website_key: self.captcha_website_key.clone(),
            email_code: self.email_code.clone(),
            locale: self.locale.clone(),
            timeout: self.timeout,
            mail_mode: self.mail_mode.clone(),
            mail_gateway_base_url: self.mail_gateway_base_url.clone(),
            mail_gateway_api_key: self.mail_gateway_api_key.clone(),
            mail_provider: self.mail_provider.clone(),
            mail_provider_mode: self.mail_provider_mode.clone(),
            mail_project_code: self.mail_project_code.clone(),
            mail_domain: self.mail_domain.clone(),
            use_freemail: false,
            freemail_base_url: None,
            freemail_admin_token: None,
            freemail_domain_index: 0,
            poll_timeout: self.poll_timeout,
            poll_interval: self.poll_interval,
            code_pattern: self.code_pattern.clone(),
            debug_email: self.debug_email,
            result_json: String::new(), // 不使用文件，直接存数据库
            test_desktop_session: self.test_desktop_session,
            proxy: self.proxy.clone(),
            use_proxy_pool: self.use_proxy_pool,
            proxy_pool_api: self.proxy_pool_api.clone(),
        }
    }
}

fn config_string(config: &HashMap<String, String>, key: &str) -> Option<String> {
    config
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn option_is_missing(value: &Option<String>) -> bool {
    value
        .as_ref()
        .map(|current| current.trim().is_empty())
        .unwrap_or(true)
}

fn apply_register_config(args: &mut RegisterArgs, config: &HashMap<String, String>) {
    if option_is_missing(&args.proxy) {
        args.proxy = config_string(config, "proxy");
    }

    if args.mail_mode.trim().is_empty() {
        if let Some(value) = config_string(config, "mail_mode") {
            args.mail_mode = value;
        }
    }
    if option_is_missing(&args.mail_gateway_base_url) {
        args.mail_gateway_base_url = config_string(config, "mail_gateway_base_url");
    }
    if option_is_missing(&args.mail_gateway_api_key) {
        args.mail_gateway_api_key = config_string(config, "mail_gateway_api_key");
    }
    if args.mail_provider.trim().is_empty() {
        if let Some(value) = config_string(config, "mail_provider") {
            args.mail_provider = value;
        }
    }
    if args.mail_provider_mode.trim().is_empty() {
        if let Some(value) = config_string(config, "mail_provider_mode") {
            args.mail_provider_mode = value;
        }
    }
    if option_is_missing(&args.mail_project_code) {
        args.mail_project_code = config_string(config, "mail_project_code");
    }
    if option_is_missing(&args.mail_domain) {
        args.mail_domain = config_string(config, "mail_domain");
    }

    if let Some(value) = config_string(config, "captcha_api_url") {
        args.captcha_api_url = value;
    }
    if let Some(value) = config_string(config, "proxy_pool_api") {
        args.proxy_pool_api = value;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub step: String,
    pub message: String,
    pub level: String,
    pub timestamp: String,
}

#[tauri::command]
pub async fn start_registration(
    app: AppHandle,
    state: State<'_, AppState>,
    args: RegisterArgs,
) -> Result<String, String> {
    let db = state.db.clone();

    // 从 DB 读取配置并注入到 args
    let mut args = args;
    if let Ok(conn) = db.lock() {
        if let Ok(config) = db::get_all_config(&conn) {
            apply_register_config(&mut args, &config);
        }
    }

    let cli_args = args.to_cli_args();

    let result = tokio::task::spawn_blocking(move || {
        orchids_core::workflow::run_with_args(cli_args, None, |log| {
            let _ = app.emit("register-log", &log);
        })
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?;

    match result {
        Ok(reg_result) => {
            let account_id = {
                let conn = db.lock().map_err(|e| e.to_string())?;
                let actual_password = reg_result.password.clone().unwrap_or_default();
                let account_id = db::insert_account(
                    &conn,
                    &NewAccount {
                        email: reg_result.email.clone(),
                        password: actual_password.clone(),
                        status: "complete".to_string(),
                        batch_id: None,
                        group_id: None,
                    },
                )
                .map_err(|e| e.to_string())?;

                db::update_account_result(
                    &conn,
                    account_id,
                    &reg_result.email,
                    reg_result.sign_up_id.as_deref(),
                    reg_result.email_code.as_deref(),
                    reg_result.register_complete,
                    reg_result.created_session_id.as_deref(),
                    reg_result.created_user_id.as_deref(),
                    reg_result.client_cookie.as_deref(),
                    reg_result.desktop_jwt.as_deref(),
                    "complete",
                    None,
                )
                .map_err(|e| e.to_string())?;
                account_id
            };

            // 注册后异步补全 plan/credits（不阻塞返回）
            {
                let email = reg_result.email.clone();
                let password = reg_result.password.clone().unwrap_or_default();
                let timeout = args.timeout;
                let proxy = args.proxy.clone();
                let db2 = db.clone();
                tokio::task::spawn_blocking(move || {
                    if let Ok(profile) = orchids_profile::fetch_plan_and_credits(&email, &password, timeout, proxy.as_deref()) {
                        if let Ok(conn) = db2.lock() {
                            let _ = db::update_account_plan_credits(
                                &conn,
                                account_id,
                                profile.plan.as_deref(),
                                profile.credits,
                            );
                        }
                    }
                });
            }

            Ok(serde_json::to_string(&reg_result).unwrap_or_default())
        }
        Err(e) => {
            // 注册失败不写入数据库，直接返回错误
            Err(e.to_string())
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchProgress {
    pub completed: usize,
    pub failed: usize,
    pub total: usize,
    pub current_email: Option<String>,
}

#[tauri::command]
pub async fn start_batch_registration(
    app: AppHandle,
    state: State<'_, AppState>,
    args: RegisterArgs,
    count: usize,
    concurrency: usize,
) -> Result<String, String> {
    let batch_id = uuid::Uuid::new_v4().to_string();
    let cancel_flag = state.batch_cancel.clone();
    cancel_flag.store(false, Ordering::SeqCst);

    let db = state.db.clone();

    // 从 DB 读取配置并注入到 args
    let mut args = args;
    if let Ok(conn) = db.lock() {
        if let Ok(config) = db::get_all_config(&conn) {
            apply_register_config(&mut args, &config);
        }
    }

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency.clamp(1, 5)));

    // 创建共享的代理池（如果启用）
    let shared_proxy_pool = if args.use_proxy_pool {
        Some(orchids_core::proxy_pool::ProxyPool::new(args.proxy_pool_api.clone()))
    } else {
        None
    };

    let batch_id_clone = batch_id.clone();
    let app_clone = app.clone();

    tokio::spawn(async move {
        let mut handles = Vec::new();
        let completed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let failed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        for _i in 0..count {
            if cancel_flag.load(Ordering::SeqCst) {
                break;
            }

            let permit = semaphore.clone().acquire_owned().await;
            if permit.is_err() {
                break;
            }
            let _permit = permit.unwrap();

            let task_args = args.clone();
            // Domain 模式已移除，不需要特殊处理

            let cli_args = task_args.to_cli_args();
            let timeout = task_args.timeout;
            let proxy = task_args.proxy.clone();
            let db = db.clone();
            let batch_id = batch_id_clone.clone();
            let app = app_clone.clone();
            let cancel = cancel_flag.clone();
            let completed = completed.clone();
            let failed = failed.clone();
            let total = count;
            let proxy_pool = shared_proxy_pool.clone();

            let handle = tokio::spawn(async move {
                if cancel.load(Ordering::SeqCst) {
                    return;
                }

                let task_app = app.clone();
                let result = tokio::task::spawn_blocking(move || {
                    orchids_core::workflow::run_with_args(cli_args, proxy_pool, |log| {
                        let _ = task_app.emit("register-log", &log);
                    })
                })
                .await;

                match result {
                    Ok(Ok(reg_result)) => {
                        let c = completed.fetch_add(1, Ordering::SeqCst) + 1;
                        let f = failed.load(Ordering::SeqCst);
                        let email_for_progress = reg_result.email.clone();
                        let password_for_profile = reg_result.password.clone().unwrap_or_default();

                        // 先推送进度，避免被后续数据库/网络操作阻塞
                        let _ = app.emit("batch-progress", BatchProgress {
                            completed: c,
                            failed: f,
                            total,
                            current_email: Some(email_for_progress.clone()),
                        });

                        if let Ok(conn) = db.lock() {
                            let aid = db::insert_account(
                                &conn,
                                &NewAccount {
                                    email: email_for_progress.clone(),
                                    password: password_for_profile.clone(),
                                    status: "complete".to_string(),
                                    batch_id: Some(batch_id.clone()),
                                    group_id: None,
                                },
                            );
                            if let Ok(aid) = aid {
                                let _ = db::update_account_result(
                                    &conn,
                                    aid,
                                    &email_for_progress,
                                    reg_result.sign_up_id.as_deref(),
                                    reg_result.email_code.as_deref(),
                                    reg_result.register_complete,
                                    reg_result.created_session_id.as_deref(),
                                    reg_result.created_user_id.as_deref(),
                                    reg_result.client_cookie.as_deref(),
                                    reg_result.desktop_jwt.as_deref(),
                                    "complete",
                                    None,
                                );

                                // 批量注册后异步补全 plan/credits，避免阻塞进度事件
                                if !password_for_profile.is_empty() {
                                    drop(conn);
                                    let db2 = db.clone();
                                    let email2 = email_for_progress.clone();
                                    let password2 = password_for_profile.clone();
                                    let proxy2 = proxy.clone();
                                    tokio::task::spawn_blocking(move || {
                                        if let Ok(profile) = orchids_profile::fetch_plan_and_credits(
                                            &email2, &password2, timeout, proxy2.as_deref(),
                                        ) {
                                            if let Ok(conn2) = db2.lock() {
                                                let _ = db::update_account_plan_credits(
                                                    &conn2,
                                                    aid,
                                                    profile.plan.as_deref(),
                                                    profile.credits,
                                                );
                                            }
                                        }
                                    });
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        let f = failed.fetch_add(1, Ordering::SeqCst) + 1;
                        let c = completed.load(Ordering::SeqCst);
                        let err_msg = e.to_string();
                        // 先推送进度，UI 能即时看到失败数量
                        let _ = app.emit("batch-progress", BatchProgress {
                            completed: c,
                            failed: f,
                            total,
                            current_email: None,
                        });
                        // 注册失败不写入数据库
                        let _ = app.emit("register-log", LogEntry {
                            step: "error".to_string(),
                            message: err_msg,
                            level: "error".to_string(),
                            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                        });
                    }
                    Err(e) => {
                        let f = failed.fetch_add(1, Ordering::SeqCst) + 1;
                        let c = completed.load(Ordering::SeqCst);
                        let err_msg = e.to_string();
                        let _ = app.emit("batch-progress", BatchProgress {
                            completed: c,
                            failed: f,
                            total,
                            current_email: None,
                        });
                        // 注册失败不写入数据库
                        let _ = app.emit("register-log", LogEntry {
                            step: "error".to_string(),
                            message: err_msg,
                            level: "error".to_string(),
                            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                        });
                    }
                }

                drop(_permit);
            });

            handles.push(handle);
        }

        for h in handles {
            let _ = h.await;
        }

        let _ = app_clone.emit("batch-complete", serde_json::json!({
            "batch_id": batch_id_clone,
            "completed": completed.load(Ordering::SeqCst),
            "failed": failed.load(Ordering::SeqCst),
            "total": count,
        }));
    });

    Ok(batch_id)
}

#[tauri::command]
pub async fn cancel_batch(state: State<'_, AppState>) -> Result<(), String> {
    state.batch_cancel.store(true, Ordering::SeqCst);
    Ok(())
}
