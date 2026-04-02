use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use regex::Regex;
use reqwest::blocking::Client;
use serde_json::{json, Value};

use crate::constants::user_agent;
use crate::errors::AppError;
use crate::http_client::{json_compact, json_or_raw, req_timeout_secs};

/// 创建 freemail 邮箱
///
/// API: POST /api/create
/// 请求参数: { "local": "myname", "domainIndex": 0 }
/// 返回: { "email": "myname@example.com", "expires": 1704067200000 }
pub fn create_freemail_inbox(
    client: &Client,
    timeout: i64,
    base_url: &str,
    admin_token: &str,
    local_part: Option<&str>,
    domain_index: Option<i32>,
) -> Result<String, AppError> {
    let url = format!("{}/api/create", base_url.trim_end_matches('/'));

    let payload = json!({
        "local": local_part,
        "domainIndex": domain_index.unwrap_or(0),
    });

    let resp = client
        .post(&url)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .header("user-agent", user_agent())
        .header("Authorization", format!("Bearer {}", admin_token))
        .timeout(req_timeout_secs(timeout))
        .body(payload.to_string())
        .send()
        .map_err(|e| AppError::Runtime(format!("创建 freemail 邮箱失败: {}", e)))?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);

    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "创建 freemail 邮箱失败: HTTP {} -> {}",
            status,
            json_compact(&data)
        )));
    }

    let email = data
        .get("email")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Runtime(format!(
            "freemail 响应缺少 email 字段: {}",
            json_compact(&data)
        )))?;

    Ok(email.to_string())
}

/// 获取邮件列表
///
/// API: GET /api/emails?mailbox=xxx&limit=20
pub fn fetch_freemail_emails(
    client: &Client,
    timeout: i64,
    base_url: &str,
    admin_token: &str,
    mailbox: &str,
    limit: i32,
) -> Result<Value, AppError> {
    let url = format!("{}/api/emails", base_url.trim_end_matches('/'));

    let resp = client
        .get(&url)
        .query(&[("mailbox", mailbox), ("limit", &limit.to_string())])
        .header("accept", "application/json")
        .header("user-agent", user_agent())
        .header("Authorization", format!("Bearer {}", admin_token))
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| AppError::Runtime(format!("获取 freemail 邮件失败: {}", e)))?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);

    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "获取 freemail 邮件失败: HTTP {} -> {}",
            status,
            json_compact(&data)
        )));
    }

    Ok(data)
}

/// 从邮件中提取验证码
fn extract_code_from_email(email_obj: &Value, code_pattern: &str) -> Option<String> {
    let regex = Regex::new(code_pattern).ok()?;

    let subject = email_obj.get("subject").and_then(Value::as_str).unwrap_or("");
    let preview = email_obj.get("preview").and_then(Value::as_str).unwrap_or("");
    let verification_code = email_obj.get("verification_code").and_then(Value::as_str);

    // 如果 API 已经提取了验证码，直接使用
    if let Some(code) = verification_code {
        if !code.is_empty() {
            return Some(code.to_string());
        }
    }

    let text = format!("{}\n{}", subject, preview);

    if let Some(caps) = regex.captures(&text) {
        if let Some(m) = caps.get(1) {
            return Some(m.as_str().to_string());
        }
        if let Some(m) = caps.get(0) {
            return Some(m.as_str().to_string());
        }
    }

    // 回退模式
    let fallback_patterns = [
        r"\b(\d{4,8})\b",
        r"(?:code|otp|verify|verification|验证码|校验码)\D{0,20}(\d{4,8})",
    ];

    for fp in fallback_patterns {
        let re = match Regex::new(fp) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let caps = match re.captures(&text) {
            Some(v) => v,
            None => continue,
        };

        let mut val = caps
            .get(1)
            .or_else(|| caps.get(0))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        val.retain(|c| c.is_ascii_digit());
        if (4..=8).contains(&val.len()) {
            return Some(val);
        }
    }

    None
}

pub struct WaitCodeOptions {
    pub debug: bool,
}

/// 等待验证码
pub fn wait_for_freemail_code(
    client: &Client,
    timeout: i64,
    wait_timeout: i64,
    poll_interval: f64,
    code_pattern: &str,
    base_url: &str,
    admin_token: &str,
    mailbox: &str,
    opts: WaitCodeOptions,
) -> Result<String, AppError> {
    if let Err(exc) = Regex::new(code_pattern) {
        return Err(AppError::Runtime(format!("验证码正则无效: {} -> {}", code_pattern, exc)));
    }

    let deadline = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        + wait_timeout as f64;

    let mut round_idx = 0;

    loop {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        if now >= deadline {
            break;
        }

        round_idx += 1;

        let emails_data = fetch_freemail_emails(client, timeout, base_url, admin_token, mailbox, 20)?;

        let emails = emails_data.as_array().ok_or_else(|| {
            AppError::Runtime(format!("freemail 返回的不是数组: {}", json_compact(&emails_data)))
        })?;

        if opts.debug {
            println!("[*] freemail 轮询第 {} 次：emails={}", round_idx, emails.len());
        }

        for email_obj in emails {
            if let Some(code) = extract_code_from_email(email_obj, code_pattern) {
                return Ok(code);
            }

            if opts.debug {
                let subject = email_obj.get("subject").and_then(Value::as_str).unwrap_or("");
                let sender = email_obj.get("sender").and_then(Value::as_str).unwrap_or("");
                println!("[*] 邮件未匹配验证码: sender={} subject={}", sender, subject);
            }
        }

        thread::sleep(Duration::from_secs_f64(poll_interval.max(0.0)));
    }

    Err(AppError::Runtime(format!(
        "在 {} 秒内未从 freemail 中提取到验证码",
        wait_timeout
    )))
}
