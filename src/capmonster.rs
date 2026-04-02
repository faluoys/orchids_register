use std::thread;
use std::time::{Duration, Instant};

use regex::Regex;
use reqwest::blocking::Client;
use serde_json::Value;

use crate::constants::user_agent;
use crate::errors::AppError;
use crate::http_client::{json_compact, json_or_raw, req_timeout_secs};

/// 使用本地打码 API 创建 Turnstile 任务
///
/// API: GET /turnstile?url=xxx&sitekey=xxx
/// 返回: { "task_id": "uuid" }
pub fn create_local_turnstile_task(
    client: &Client,
    api_base_url: &str,
    website_url: &str,
    website_key: &str,
    timeout: i64,
) -> Result<String, AppError> {
    let url = format!("{}/turnstile", api_base_url.trim_end_matches('/'));

    let resp = client
        .get(&url)
        .query(&[("url", website_url), ("sitekey", website_key)])
        .header("accept", "application/json")
        .header("user-agent", user_agent())
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| AppError::Runtime(format!("创建打码任务失败: {}", e)))?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);

    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "创建打码任务失败: HTTP {} -> {}",
            status,
            json_compact(&data)
        )));
    }

    let task_id = data
        .get("taskId")
        .or_else(|| data.get("task_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Runtime(format!(
            "打码 API 响应缺少 taskId: {}",
            json_compact(&data)
        )))?;

    Ok(task_id.to_string())
}

/// 获取本地打码任务结果
///
/// API: GET /result?id=xxx
/// 返回: { "solution": { "token": "xxx" } } 或仍在处理中
pub fn get_local_task_result(
    client: &Client,
    api_base_url: &str,
    task_id: &str,
    timeout: i64,
) -> Result<Value, AppError> {
    let url = format!("{}/result", api_base_url.trim_end_matches('/'));

    let resp = client
        .get(&url)
        .query(&[("id", task_id)])
        .header("accept", "application/json")
        .header("user-agent", user_agent())
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| AppError::Runtime(format!("获取打码结果失败: {}", e)))?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);

    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "获取打码结果失败: HTTP {} -> {}",
            status,
            json_compact(&data)
        )));
    }

    Ok(data)
}

pub fn discover_turnstile_sitekey(client: &Client, website_url: &str, timeout: i64) -> Result<Option<String>, AppError> {
    let resp = client
        .get(website_url)
        .header("accept", "text/html,application/xhtml+xml")
        .header("user-agent", user_agent())
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| AppError::Runtime(e.to_string()))?;

    if resp.status().as_u16() >= 400 {
        return Ok(None);
    }

    let html = resp.text().unwrap_or_default();
    let patterns = [
        r#"data-sitekey=[\"']([0-9A-Za-z_-]{10,})[\"']"#,
        r#"[\"']sitekey[\"']\s*:\s*[\"']([^\"']+)[\"']"#,
        r#"turnstile\.render\([^\)]*?[\"']([0-9A-Za-z_-]{10,})[\"']"#,
    ];

    for pattern in patterns {
        let re = Regex::new(pattern).map_err(|e| AppError::Runtime(e.to_string()))?;
        if let Some(caps) = re.captures(&html) {
            if let Some(m) = caps.get(1) {
                return Ok(Some(m.as_str().to_string()));
            }
        }
    }

    Ok(None)
}

/// 使用本地打码 API 求解 Turnstile
pub fn solve_turnstile_with_local_api(
    client: &Client,
    api_base_url: &str,
    website_url: &str,
    website_key: &str,
    timeout: i64,
    wait_timeout: i64,
    poll_interval: f64,
) -> Result<String, AppError> {
    let task_id = create_local_turnstile_task(
        client,
        api_base_url,
        website_url,
        website_key,
        timeout,
    )?;

    let deadline = Instant::now() + Duration::from_secs(wait_timeout.max(0) as u64);
    while Instant::now() < deadline {
        let result = get_local_task_result(client, api_base_url, &task_id, timeout)?;

        // 检查是否有 solution.token
        if let Some(solution) = result.get("solution") {
            if let Some(token) = solution.get("token").and_then(Value::as_str) {
                if !token.is_empty() && token != "CAPTCHA_FAIL" {
                    return Ok(token.to_string());
                } else if token == "CAPTCHA_FAIL" {
                    return Err(AppError::Runtime("打码失败: CAPTCHA_FAIL".to_string()));
                }
            }
        }

        // 如果没有 solution 或 token 为空，继续等待
        thread::sleep(Duration::from_secs_f64(poll_interval.max(0.0)));
    }

    Err(AppError::Runtime(format!(
        "打码 API 在 {} 秒内未完成 Turnstile 求解",
        wait_timeout
    )))
}
