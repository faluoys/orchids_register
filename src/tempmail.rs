use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::DateTime;
use html_escape::decode_html_entities;
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::{json, Value};

use crate::constants::{user_agent, TEMPMAIL_BASE};
use crate::errors::AppError;
use crate::http_client::{json_compact, json_or_raw, req_timeout_secs};

fn tempmail_headers(api_key: Option<&str>) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("accept", "application/json".to_string()),
        ("user-agent", user_agent().to_string()),
    ];
    if let Some(key) = api_key {
        if !key.is_empty() {
            headers.push(("authorization", format!("Bearer {}", key)));
        }
    }
    headers
}

pub fn create_tempmail_inbox(
    client: &Client,
    timeout: i64,
    api_key: Option<&str>,
    domain: Option<&str>,
    prefix: Option<&str>,
) -> Result<(String, String), AppError> {
    let url = format!("{TEMPMAIL_BASE}/inbox/create");
    let payload = json!({
        "domain": domain,
        "prefix": prefix,
    });

    let mut req = client
        .post(url)
        .header("content-type", "application/json")
        .timeout(req_timeout_secs(timeout));

    for (k, v) in tempmail_headers(api_key) {
        req = req.header(k, v);
    }

    let resp = req
        .body(payload.to_string())
        .send()
        .map_err(|e| AppError::Runtime(e.to_string()))?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);
    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "创建临时邮箱失败: HTTP {} -> {}",
            status,
            json_compact(&data)
        )));
    }

    let address = data.get("address").and_then(Value::as_str).map(|s| s.to_string());
    let token = data.get("token").and_then(Value::as_str).map(|s| s.to_string());

    match (address, token) {
        (Some(a), Some(t)) => Ok((a, t)),
        _ => Err(AppError::Runtime(format!(
            "临时邮箱响应缺少 address/token: {}",
            json_compact(&data)
        ))),
    }
}

pub fn fetch_tempmail_emails(
    client: &Client,
    token: &str,
    timeout: i64,
    api_key: Option<&str>,
) -> Result<Value, AppError> {
    let url = format!("{TEMPMAIL_BASE}/inbox");
    let mut req = client
        .get(url)
        .query(&[("token", token)])
        .timeout(req_timeout_secs(timeout));

    for (k, v) in tempmail_headers(api_key) {
        req = req.header(k, v);
    }

    let resp = req
        .send()
        .map_err(|e| AppError::Runtime(e.to_string()))?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);
    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "拉取临时邮箱失败: HTTP {} -> {}",
            status,
            json_compact(&data)
        )));
    }

    Ok(data)
}

fn to_unix_ms(ts: &Value) -> i64 {
    if let Some(n) = ts.as_i64() {
        return if n < 10_000_000_000 { n * 1000 } else { n };
    }
    if let Some(n) = ts.as_f64() {
        return if n < 10_000_000_000f64 {
            (n * 1000f64) as i64
        } else {
            n as i64
        };
    }

    if let Some(s) = ts.as_str() {
        let s = s.trim();
        if s.is_empty() {
            return 0;
        }

        if s.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(num) = s.parse::<i64>() {
                return if num < 10_000_000_000 { num * 1000 } else { num };
            }
        }

        let iso = s.replace('Z', "+00:00");
        if let Ok(dt) = DateTime::parse_from_rfc3339(&iso) {
            return dt.timestamp_millis();
        }
    }

    0
}

fn extract_tempmail_emails(inbox_data: &Value) -> Vec<Value> {
    let mut candidates = Vec::new();

    for key in ["emails", "email", "messages", "mails", "items"] {
        if let Some(arr) = inbox_data.get(key).and_then(Value::as_array) {
            candidates.extend(arr.iter().cloned());
        }
    }

    for parent_key in ["data", "response", "result"] {
        if let Some(parent) = inbox_data.get(parent_key).and_then(Value::as_object) {
            for key in ["emails", "email", "messages", "mails", "items"] {
                if let Some(arr) = parent.get(key).and_then(Value::as_array) {
                    candidates.extend(arr.iter().cloned());
                }
            }
        }
    }

    candidates.into_iter().filter(|v| v.is_object()).collect()
}

fn extract_code_from_email(email_obj: &Value, code_pattern: &str) -> Option<String> {
    let regex = Regex::new(code_pattern).ok()?;

    let subject = email_obj
        .get("subject")
        .or_else(|| email_obj.get("title"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let body = email_obj
        .get("body")
        .or_else(|| email_obj.get("text"))
        .or_else(|| email_obj.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let html_content = email_obj
        .get("html")
        .or_else(|| email_obj.get("htmlBody"))
        .or_else(|| email_obj.get("html_body"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let html_re = Regex::new(r"<[^>]+>").ok()?;
    let html_text = html_re.replace_all(html_content, " ");
    let html_unescaped = decode_html_entities(&html_text).to_string();

    let text = format!("{}\n{}\n{}", subject, body, html_unescaped);
    let text_lower = text.to_lowercase();

    if let Some(caps) = regex.captures(&text) {
        if let Some(m) = caps.get(1) {
            return Some(m.as_str().to_string());
        }
        if let Some(m) = caps.get(0) {
            return Some(m.as_str().to_string());
        }
    }

    let fallback_patterns = [
        r"\b(\d{4,8})\b",
        r"(?:code|otp|verify|verification|验证码|校验码)\D{0,20}(\d{4,8})",
        r"(\d(?:[\s\-]\d){5,7})",
    ];

    for fp in fallback_patterns {
        let re = match Regex::new(fp) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let caps = match re.captures(&text_lower) {
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

pub struct WaitCodeOptions<'a> {
    pub expected_to: Option<&'a str>,
    pub not_older_than_ms: Option<i64>,
    pub api_key: Option<&'a str>,
    pub debug: bool,
}

pub fn wait_for_tempmail_code(
    client: &Client,
    token: &str,
    timeout: i64,
    wait_timeout: i64,
    poll_interval: f64,
    code_pattern: &str,
    opts: WaitCodeOptions<'_>,
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
    let mut last_nonempty_email: Option<Value> = None;

    loop {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        if now >= deadline {
            break;
        }

        round_idx += 1;

        let inbox_data = fetch_tempmail_emails(client, token, timeout, opts.api_key)?;

        if inbox_data.get("expired").and_then(Value::as_bool) == Some(true) {
            return Err(AppError::Runtime("临时邮箱已过期，请重新创建后重试".to_string()));
        }

        let mut emails = extract_tempmail_emails(&inbox_data);

        if opts.debug {
            let expired = inbox_data.get("expired").cloned().unwrap_or(Value::Null);
            println!(
                "[*] tempmail 轮询第 {} 次：emails={} expired={}",
                round_idx,
                emails.len(),
                expired
            );
        }

        emails.sort_by_key(|x| {
            let ts = x
                .get("date")
                .or_else(|| x.get("createdAt"))
                .or_else(|| x.get("created_at"))
                .or_else(|| x.get("timestamp"))
                .cloned()
                .unwrap_or(Value::Null);
            -to_unix_ms(&ts)
        });

        for email_obj in &emails {
            if !email_obj.is_object() {
                continue;
            }

            last_nonempty_email = Some(email_obj.clone());

            let email_ts = to_unix_ms(
                email_obj
                    .get("date")
                    .or_else(|| email_obj.get("createdAt"))
                    .or_else(|| email_obj.get("created_at"))
                    .or_else(|| email_obj.get("timestamp"))
                    .unwrap_or(&Value::Null),
            );

            if let Some(not_older_than_ms) = opts.not_older_than_ms {
                if email_ts != 0 && email_ts < (not_older_than_ms - 120_000) {
                    if opts.debug {
                        println!(
                            "[*] 跳过旧邮件: email_ts={} not_older_than_ms={}",
                            email_ts, not_older_than_ms
                        );
                    }
                    continue;
                }
            }

            if let Some(expected_to) = opts.expected_to {
                let to_addr = email_obj
                    .get("to")
                    .or_else(|| email_obj.get("to_address"))
                    .or_else(|| email_obj.get("recipient"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();

                if !to_addr.is_empty() && !to_addr.to_lowercase().contains(&expected_to.to_lowercase()) {
                    continue;
                }
            }

            if let Some(code) = extract_code_from_email(email_obj, code_pattern) {
                return Ok(code);
            }

            if opts.debug {
                let subject_dbg = email_obj
                    .get("subject")
                    .or_else(|| email_obj.get("title"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let to_dbg = email_obj
                    .get("to")
                    .or_else(|| email_obj.get("to_address"))
                    .or_else(|| email_obj.get("recipient"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let body_dbg_raw = email_obj
                    .get("body")
                    .or_else(|| email_obj.get("text"))
                    .or_else(|| email_obj.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let ws_re = Regex::new(r"\s+").ok();
                let body_dbg = ws_re
                    .as_ref()
                    .map(|r| r.replace_all(body_dbg_raw, " ").to_string())
                    .unwrap_or_else(|| body_dbg_raw.to_string());
                let body_dbg = body_dbg.chars().take(160).collect::<String>();
                println!(
                    "[*] 邮件未匹配验证码: to={} subject={} body={}",
                    to_dbg, subject_dbg, body_dbg
                );
            }
        }

        if opts.debug && !emails.is_empty() {
            if let Some(sample) = emails.first().and_then(Value::as_object) {
                let mut keys: Vec<String> = sample.keys().cloned().collect();
                keys.sort();
                println!("[*] 最近一封邮件字段: {}", keys.join(", "));
            }
        }

        thread::sleep(Duration::from_secs_f64(poll_interval.max(0.0)));
    }

    if let Some(last) = last_nonempty_email {
        let sample_subject = last
            .get("subject")
            .or_else(|| last.get("title"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let sample_to = last
            .get("to")
            .or_else(|| last.get("to_address"))
            .or_else(|| last.get("recipient"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let sample_body_raw = last
            .get("body")
            .or_else(|| last.get("text"))
            .or_else(|| last.get("content"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let ws_re = Regex::new(r"\s+").ok();
        let sample_body = ws_re
            .as_ref()
            .map(|r| r.replace_all(sample_body_raw, " ").to_string())
            .unwrap_or_else(|| sample_body_raw.to_string());
        let sample_body = sample_body.chars().take(200).collect::<String>();

        return Err(AppError::Runtime(format!(
            "在 {} 秒内未从临时邮箱中提取到验证码；最后一封邮件摘要: to={}, subject={}, body={}",
            wait_timeout, sample_to, sample_subject, sample_body
        )));
    }

    Err(AppError::Runtime(format!(
        "在 {} 秒内未从临时邮箱中提取到验证码（期间收件箱为空）",
        wait_timeout
    )))
}
