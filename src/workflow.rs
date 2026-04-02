use std::time::{SystemTime, UNIX_EPOCH};

use clap::CommandFactory;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::capmonster::{discover_turnstile_sitekey, solve_turnstile_with_local_api};
use crate::clerk::{
    attempt_email_verification, create_sign_up, extract_client_cookie, extract_signup_id, init_clerk_environment,
    pick_summary_fields, prepare_email_verification,
};
use crate::cli::{parse_args, Args};
use crate::constants::generate_random_password;
use crate::desktop::test_desktop_session;
use crate::errors::AppError;
use crate::http_client::{create_client, json_compact};
use crate::inbox_gateway::{
    acquire_inbox, gateway_poll_http_timeout_secs, poll_code as poll_gateway_code, release_inbox, AcquireInboxResponse, GatewaySettings, PollCodeRequest,
};
use crate::proxy_pool::ProxyPool;
use crate::result_store::save_result_json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub step: String,
    pub message: String,
    pub level: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResult {
    pub email: String,
    pub password: Option<String>,
    pub sign_up_id: Option<String>,
    pub email_code: Option<String>,
    pub register_complete: bool,
    pub created_session_id: Option<String>,
    pub created_user_id: Option<String>,
    pub client_cookie: Option<String>,
    pub desktop_touch_ok: Option<bool>,
    pub desktop_tokens_ok: Option<bool>,
    pub desktop_session_usable: Option<bool>,
    pub desktop_jwt: Option<String>,
}

fn usage_error(err_msg: &str) -> AppError {
    let mut cmd = Args::command();
    let mut rendered = Vec::new();
    let _ = cmd.write_long_help(&mut rendered);
    let help_text = String::from_utf8_lossy(&rendered);

    AppError::Usage(format!("error: {}\n\nUsage: orchids-auto-register [OPTIONS]\n\n{}", err_msg, help_text))
}

pub fn run() -> Result<i32, AppError> {
    let args = parse_args();
    let result = run_with_args(args, None, |log| {
        match log.level.as_str() {
            "error" => eprintln!("[{}] {} {}", log.timestamp, log.step, log.message),
            _ => println!("[{}] {} {}", log.timestamp, log.step, log.message),
        }
    });
    match result {
        Ok(_) => Ok(0),
        Err(e) => Err(e),
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn timestamp() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn finalize_with_gateway_release(
    flow_result: Result<RegistrationResult, AppError>,
    gateway_session: Option<&AcquireInboxResponse>,
    mail_client: &reqwest::blocking::Client,
    timeout: i64,
    gateway_settings: &GatewaySettings,
) -> Result<RegistrationResult, AppError> {
    if let Some(session) = gateway_session {
        if let Err(release_err) = release_inbox(mail_client, timeout, gateway_settings, &session.session_id) {
            eprintln!(
                "[warn] 释放 mail-gateway inbox {} 失败: {}",
                session.session_id, release_err
            );
        }
    }

    flow_result
}

pub fn run_with_args<F: Fn(LogEntry)>(args: Args, shared_proxy_pool: Option<ProxyPool>, on_log: F) -> Result<RegistrationResult, AppError> {
    // 如果使用代理池，尝试最多 3 次（使用不同的代理）
    let max_attempts = if args.use_proxy_pool { 3 } else { 1 };

    for attempt in 1..=max_attempts {
        if attempt > 1 {
            on_log(LogEntry {
                step: "[重试]".to_string(),
                message: format!("第 {} 次尝试，获取新代理...", attempt),
                level: "info".to_string(),
                timestamp: timestamp(),
            });
        }

        match run_with_args_internal(args.clone(), shared_proxy_pool.clone(), &on_log) {
            Ok(result) => return Ok(result),
            Err(e) => {
                let err_msg = e.to_string();
                // 检查是否是代理相关的错误
                let is_proxy_error = err_msg.contains("error sending request")
                    || err_msg.contains("connection")
                    || err_msg.contains("timeout")
                    || err_msg.contains("builder error");

                if is_proxy_error && attempt < max_attempts && args.use_proxy_pool {
                    on_log(LogEntry {
                        step: "[重试]".to_string(),
                        message: format!("代理失败: {}，将使用新代理重试", err_msg),
                        level: "warn".to_string(),
                        timestamp: timestamp(),
                    });
                    continue;
                } else {
                    return Err(e);
                }
            }
        }
    }

    Err(AppError::Runtime("所有重试均失败".to_string()))
}

fn run_with_args_internal<F: Fn(LogEntry)>(args: Args, shared_proxy_pool: Option<ProxyPool>, on_log: &F) -> Result<RegistrationResult, AppError> {
    // 如果使用代理池，先获取一个代理
    let proxy = if args.use_proxy_pool {
        let pool = shared_proxy_pool.unwrap_or_else(|| ProxyPool::new(args.proxy_pool_api.clone()));
        let temp_client = reqwest::blocking::Client::new();
        let proxy_addr = pool.get_proxy(&temp_client, args.timeout)?;
        on_log(LogEntry {
            step: "[*]".to_string(),
            message: format!("从代理池获取代理: {}", proxy_addr),
            level: "info".to_string(),
            timestamp: timestamp(),
        });
        Some(proxy_addr)
    } else {
        args.proxy.clone()
    };

    let (client, cookie_store) = create_client(proxy.as_deref())?;
    let (mail_client, _) = create_client(None)?;
    let gateway_settings = GatewaySettings::from_args(&args);
    gateway_settings.validate()?;
    if !gateway_settings.mode.eq_ignore_ascii_case("gateway") && !gateway_settings.mode.eq_ignore_ascii_case("manual") {
        return Err(usage_error("mail_mode 仅支持 gateway 或 manual"));
    }

    let mut gateway_session: Option<AcquireInboxResponse> = None;
    let mut email = args.email.clone();
    let mut captcha_token = args.captcha_token.clone();
    let password = args.password.clone().unwrap_or_else(|| {
        let p = generate_random_password();
        on_log(LogEntry {
            step: "[*]".to_string(),
            message: format!("已自动生成随机密码: {}", p),
            level: "info".to_string(),
            timestamp: timestamp(),
        });
        p
    });

    let flow_result = (|| -> Result<RegistrationResult, AppError> {
        let mut result = RegistrationResult {
            email: String::new(),
            password: Some(password.clone()),
            sign_up_id: None,
            email_code: None,
            register_complete: false,
            created_session_id: None,
            created_user_id: None,
            client_cookie: None,
            desktop_touch_ok: None,
            desktop_tokens_ok: None,
            desktop_session_usable: None,
            desktop_jwt: None,
        };

        let mut result_payload = json!({
            "email": Value::Null,
            "password": &password,
            "sign_up_id": Value::Null,
            "email_code": Value::Null,
            "register_complete": false,
            "created_session_id": Value::Null,
            "created_user_id": Value::Null,
            "client_cookie": Value::Null,
            "desktop_session_test": Value::Null,
        });

        if email.is_none() {
            if gateway_settings.mode.eq_ignore_ascii_case("manual") {
                return Err(usage_error("mail_mode=manual 时必须提供 --email"));
            }

            if gateway_settings.enabled() {
                on_log(LogEntry {
                    step: "[0/4]".to_string(),
                    message: "通过 mail-gateway 申请邮箱...".to_string(),
                    level: "info".to_string(),
                    timestamp: timestamp(),
                });

                let acquired = acquire_inbox(&mail_client, args.timeout, &gateway_settings)?;
                let address = acquired.address.clone();
                let session_id = acquired.session_id.clone();
                email = Some(address.clone());
                gateway_session = Some(acquired);

                on_log(LogEntry {
                    step: "[0/4]".to_string(),
                    message: format!("mail-gateway 邮箱: {} (session={})", address, session_id),
                    level: "info".to_string(),
                    timestamp: timestamp(),
                });
            } else {
                return Err(usage_error("mail_mode=gateway 时必须提供 mail-gateway 配置，或手动传入 --email"));
            }
        }

        result.email = email.clone().unwrap_or_default();
        if let Some(obj) = result_payload.as_object_mut() {
            obj.insert(
                "email".to_string(),
                email.clone().map(Value::String).unwrap_or(Value::Null),
            );
        }

    on_log(LogEntry {
        step: "[1/4]".to_string(),
        message: "初始化 Clerk 环境...".to_string(),
        level: "info".to_string(),
        timestamp: timestamp(),
    });
    let _ = init_clerk_environment(&client, args.timeout)?;

    if captcha_token.is_none() {
        if !args.use_capmonster {
            return Err(usage_error(
                "必须提供 --captcha-token，或启用 --use-capmonster 自动求解",
            ));
        }

        let mut website_key = args.captcha_website_key.clone();
        if website_key.is_empty() {
            on_log(LogEntry {
                step: "[*]".to_string(),
                message: "尝试自动发现 Turnstile sitekey...".to_string(),
                level: "info".to_string(),
                timestamp: timestamp(),
            });
            if let Some(found) = discover_turnstile_sitekey(&client, &args.captcha_website_url, args.timeout)? {
                website_key = found.clone();
                on_log(LogEntry {
                    step: "[*]".to_string(),
                    message: format!("已自动发现 sitekey: {}", found),
                    level: "info".to_string(),
                    timestamp: timestamp(),
                });
            }
        }

        if website_key.is_empty() {
            return Err(usage_error(
                "未提供 --captcha-website-key，且自动发现 sitekey 失败",
            ));
        }

        on_log(LogEntry {
            step: "[*]".to_string(),
            message: "使用本地打码 API 求解 Turnstile...".to_string(),
            level: "info".to_string(),
            timestamp: timestamp(),
        });
        captcha_token = Some(solve_turnstile_with_local_api(
            &client,
            &args.captcha_api_url,
            &args.captcha_website_url,
            &website_key,
            args.timeout,
            args.captcha_timeout,
            args.captcha_poll_interval,
        )?);
        on_log(LogEntry {
            step: "[*]".to_string(),
            message: "已获取 captcha_token".to_string(),
            level: "info".to_string(),
            timestamp: timestamp(),
        });
    }

    on_log(LogEntry {
        step: "[2/4]".to_string(),
        message: "创建 sign_up...".to_string(),
        level: "info".to_string(),
        timestamp: timestamp(),
    });
    let sign_up_payload = create_sign_up(
        &client,
        email.as_deref().unwrap_or_default(),
        &password,
        captcha_token.as_deref().unwrap_or_default(),
        &args.locale,
        args.timeout,
    )?;

    let sign_up_id = match extract_signup_id(&sign_up_payload) {
        Some(v) => v,
        None => {
            on_log(LogEntry {
                step: "[2/4]".to_string(),
                message: format!(
                    "sign_up 响应异常 (无 sign_up_id): {}",
                    json_compact(&pick_summary_fields(&sign_up_payload))
                ),
                level: "error".to_string(),
                timestamp: timestamp(),
            });
            return Err(AppError::Runtime("未能从响应中提取 sign_up_id，请检查返回结构".to_string()));
        }
    };

    result.sign_up_id = Some(sign_up_id.clone());
    if let Some(obj) = result_payload.as_object_mut() {
        obj.insert("sign_up_id".to_string(), Value::String(sign_up_id.clone()));
    }
    on_log(LogEntry {
        step: "[2/4]".to_string(),
        message: format!("sign_up_id: {}", sign_up_id),
        level: "info".to_string(),
        timestamp: timestamp(),
    });

    let prepare_start_ms = now_ms() - 5_000;
    on_log(LogEntry {
        step: "[3/4]".to_string(),
        message: "发送邮箱验证码...".to_string(),
        level: "info".to_string(),
        timestamp: timestamp(),
    });
    let prepare_payload = prepare_email_verification(&client, &sign_up_id, args.timeout)?;
    on_log(LogEntry {
        step: "[3/4]".to_string(),
        message: format!(
            "prepare_verification 响应摘要: {}",
            json_compact(&pick_summary_fields(&prepare_payload))
        ),
        level: "info".to_string(),
        timestamp: timestamp(),
    });

    let mut email_code = args.email_code.clone();
    if email_code.is_none() {
        if let Some(session) = gateway_session.as_ref() {
            on_log(LogEntry {
                step: "[4/4]".to_string(),
                message: "通过 mail-gateway 轮询邮箱验证码...".to_string(),
                level: "info".to_string(),
                timestamp: timestamp(),
            });

            let poll_response = poll_gateway_code(
                &mail_client,
                gateway_poll_http_timeout_secs(args.timeout, args.poll_timeout),
                &gateway_settings,
                &session.session_id,
                &PollCodeRequest {
                    timeout_seconds: args.poll_timeout,
                    interval_seconds: args.poll_interval,
                    code_pattern: args.code_pattern.clone(),
                    after_ts: Some(prepare_start_ms),
                },
            )?;

            let code = poll_response
                .code
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    AppError::Runtime(format!(
                        "mail-gateway poll-code 未返回验证码: status={}, summary={}",
                        poll_response.status,
                        serde_json::to_string(&poll_response.summary).unwrap_or_else(|_| "{}".to_string())
                    ))
                })?;

            email_code = Some(code.clone());
            result.email_code = Some(code.clone());
            if let Some(obj) = result_payload.as_object_mut() {
                obj.insert("email_code".to_string(), Value::String(code.clone()));
                obj.insert(
                    "mail_gateway_poll".to_string(),
                    json!({
                        "status": poll_response.status,
                        "message_id": poll_response.message_id,
                        "received_at": poll_response.received_at,
                        "summary": poll_response.summary,
                    }),
                );
            }
            on_log(LogEntry {
                step: "[4/4]".to_string(),
                message: format!("已提取验证码: {}", code),
                level: "info".to_string(),
                timestamp: timestamp(),
            });
        }
    }

    if email_code.is_none() {
        on_log(LogEntry {
            step: "[4/4]".to_string(),
            message: "未提供验证码，且当前流程没有可用的自动拉码会话".to_string(),
            level: "warn".to_string(),
            timestamp: timestamp(),
        });
        if !args.result_json.is_empty() {
            let output_path = save_result_json(&args.result_json, &result_payload)?;
            on_log(LogEntry {
                step: "[4/4]".to_string(),
                message: format!("注册结果已写入: {}", output_path),
                level: "info".to_string(),
                timestamp: timestamp(),
            });
        }
        return Ok(result);
    }

    if let Some(code) = &email_code {
        result.email_code = Some(code.clone());
        if let Some(obj) = result_payload.as_object_mut() {
            obj.insert("email_code".to_string(), Value::String(code.clone()));
        }
    }

    on_log(LogEntry {
        step: "[4/4]".to_string(),
        message: "提交邮箱验证码...".to_string(),
        level: "info".to_string(),
        timestamp: timestamp(),
    });
    let verify_payload = attempt_email_verification(
        &client,
        &sign_up_id,
        email_code.as_deref().unwrap_or_default(),
        args.timeout,
    )?;
    on_log(LogEntry {
        step: "[4/4]".to_string(),
        message: {
            let status = verify_payload
                .get("response")
                .and_then(|r| r.get("status"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let session_id = verify_payload
                .get("response")
                .and_then(|r| r.get("created_session_id"))
                .and_then(Value::as_str)
                .unwrap_or("-");
            let user_id = verify_payload
                .get("response")
                .and_then(|r| r.get("created_user_id"))
                .and_then(Value::as_str)
                .unwrap_or("-");
            format!(
                "attempt_verification 结果: status={}, session_id={}, user_id={}",
                status, session_id, user_id
            )
        },
        level: "info".to_string(),
        timestamp: timestamp(),
    });

    if let Some(response_obj) = verify_payload.get("response").and_then(Value::as_object) {
        let session_id = response_obj.get("created_session_id").and_then(Value::as_str).map(|s| s.to_string());
        let user_id = response_obj.get("created_user_id").and_then(Value::as_str).map(|s| s.to_string());
        let is_complete = response_obj.get("status").and_then(Value::as_str) == Some("complete");

        result.created_session_id = session_id.clone();
        result.created_user_id = user_id.clone();
        result.register_complete = is_complete;

        if let Some(obj) = result_payload.as_object_mut() {
            obj.insert(
                "created_session_id".to_string(),
                session_id.clone().map(Value::String).unwrap_or(Value::Null),
            );
            obj.insert(
                "created_user_id".to_string(),
                user_id.map(Value::String).unwrap_or(Value::Null),
            );
            obj.insert("register_complete".to_string(), Value::Bool(is_complete));
        }
    }

    let client_cookie = extract_client_cookie(&cookie_store);
    result.client_cookie = client_cookie.clone();
    if let Some(obj) = result_payload.as_object_mut() {
        obj.insert(
            "client_cookie".to_string(),
            client_cookie.clone().map(Value::String).unwrap_or(Value::Null),
        );
    }

    if args.test_desktop_session {
        let created_session_id = result.created_session_id.clone().or_else(|| {
            result_payload
                .get("created_session_id")
                .and_then(Value::as_i64)
                .map(|n| n.to_string())
        });

        if let Some(session_id) = created_session_id {
            on_log(LogEntry {
                step: "[5/5]".to_string(),
                message: "测试桌面应用会话...".to_string(),
                level: "info".to_string(),
                timestamp: timestamp(),
            });
            let desktop_result = test_desktop_session(&client, &session_id, args.timeout)?;

            result.desktop_touch_ok = desktop_result.get("touch_ok").and_then(Value::as_bool);
            result.desktop_tokens_ok = desktop_result.get("tokens_ok").and_then(Value::as_bool);
            result.desktop_session_usable = desktop_result.get("session_usable_for_desktop").and_then(Value::as_bool);
            result.desktop_jwt = desktop_result.get("jwt").and_then(Value::as_str).map(|s| s.to_string());

            let reduced = json!({
                "touch_ok": desktop_result.get("touch_ok").cloned().unwrap_or(Value::Null),
                "tokens_ok": desktop_result.get("tokens_ok").cloned().unwrap_or(Value::Null),
                "session_usable_for_desktop": desktop_result.get("session_usable_for_desktop").cloned().unwrap_or(Value::Null),
                "jwt": desktop_result.get("jwt").cloned().unwrap_or(Value::Null),
                "touch_http_status": desktop_result.get("touch_http_status").cloned().unwrap_or(Value::Null),
                "tokens_http_status": desktop_result.get("tokens_http_status").cloned().unwrap_or(Value::Null),
            });

            if let Some(obj) = result_payload.as_object_mut() {
                obj.insert("desktop_session_test".to_string(), reduced);
            }

            on_log(LogEntry {
                step: "[5/5]".to_string(),
                message: format!(
                    "桌面会话测试结果: {}",
                    json_compact(&json!({
                        "touch_ok": desktop_result.get("touch_ok").cloned().unwrap_or(Value::Null),
                        "tokens_ok": desktop_result.get("tokens_ok").cloned().unwrap_or(Value::Null),
                        "session_usable_for_desktop": desktop_result.get("session_usable_for_desktop").cloned().unwrap_or(Value::Null),
                    }))
                ),
                level: "info".to_string(),
                timestamp: timestamp(),
            });
        }
    }

    if !args.result_json.is_empty() {
        let output_path = save_result_json(&args.result_json, &result_payload)?;
        on_log(LogEntry {
            step: "完成".to_string(),
            message: format!("注册结果已写入: {}", output_path),
            level: "info".to_string(),
            timestamp: timestamp(),
        });
    }

    Ok(result)
    })();

    finalize_with_gateway_release(
        flow_result,
        gateway_session.as_ref(),
        &mail_client,
        gateway_poll_http_timeout_secs(args.timeout, args.poll_timeout),
        &gateway_settings,
    )
}




