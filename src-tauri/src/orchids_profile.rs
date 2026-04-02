use serde_json::Value;

use orchids_core::constants::user_agent;
use orchids_core::http_client::{create_client, json_or_raw, req_timeout_secs};

const CLERK_BASE: &str = "https://clerk.orchids.app";
const ORCHIDS_WEB: &str = "https://www.orchids.app";
const API_VERSION: &str = "2025-11-10";
const JS_VERSION: &str = "5.125.3";

const NEXT_ACTION_GET_USER: &str = "7f7eaa0b21b97b6937fdbab6aa91f1edd3d887506d";
const NEXT_ACTION_PROFILE_INIT: &str = "7f8dc938d6fb260138805a8ad7d07a4cafc6d04e28";
const NEXT_ROUTER_STATE_TREE: &str =
    "%5B%22%22%2C%7B%22children%22%3A%5B%22__PAGE__%22%2C%7B%7D%2Cnull%2Cnull%5D%7D%2Cnull%2Cnull%2Ctrue%5D";

#[derive(Debug, Clone)]
pub struct ProfileResult {
    pub plan: Option<String>,
    pub credits: Option<i64>,
}

pub fn fetch_plan_and_credits(email: &str, password: &str, timeout: i64, proxy: Option<&str>) -> Result<ProfileResult, String> {
    let (client, _) = create_client(proxy).map_err(|e| e.to_string())?;

    init_environment(&client, timeout)?;
    get_client(&client, timeout)?;
    let sign_in = sign_in_with_password(&client, email, password, timeout)?;
    let session_id = extract_created_session_id(&sign_in)
        .ok_or_else(|| "登录成功但未获取到 created_session_id".to_string())?;
    let touch_data = touch_session(&client, &session_id, timeout)?;
    let user_id = extract_user_id_from_touch(&touch_data)
        .ok_or_else(|| "touch 成功但未获取到 user_id".to_string())?;
    let deployment_id = get_home_deployment_id(&client, timeout);

    // 实测部分账号首次直接拉取会 500，需要先初始化 profile 状态再重试
    if let Ok(profile) = fetch_profile(&client, &user_id, timeout, deployment_id.as_deref()) {
        return Ok(profile);
    }

    init_profile_state(&client, timeout, deployment_id.as_deref())?;
    std::thread::sleep(std::time::Duration::from_millis(800));
    if let Ok(profile) = fetch_profile(&client, &user_id, timeout, deployment_id.as_deref()) {
        return Ok(profile);
    }

    // 再补一次初始化+重试，兼容后端状态传播慢的场景
    init_profile_state(&client, timeout, deployment_id.as_deref())?;
    std::thread::sleep(std::time::Duration::from_millis(1000));
    fetch_profile(&client, &user_id, timeout, deployment_id.as_deref())
}

fn clerk_params() -> [(&'static str, &'static str); 2] {
    [
        ("__clerk_api_version", API_VERSION),
        ("_clerk_js_version", JS_VERSION),
    ]
}

fn init_environment(client: &reqwest::blocking::Client, timeout: i64) -> Result<(), String> {
    let url = format!("{CLERK_BASE}/v1/environment");
    let resp = client
        .get(url)
        .query(&clerk_params())
        .header("accept", "*/*")
        .header("origin", ORCHIDS_WEB)
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    if status >= 400 {
        let data = json_or_raw(resp);
        return Err(format!("初始化 environment 失败: HTTP {} -> {}", status, data));
    }
    Ok(())
}

fn get_client(client: &reqwest::blocking::Client, timeout: i64) -> Result<(), String> {
    let url = format!("{CLERK_BASE}/v1/client");
    let resp = client
        .get(url)
        .query(&clerk_params())
        .header("accept", "*/*")
        .header("origin", ORCHIDS_WEB)
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    if status >= 400 {
        let data = json_or_raw(resp);
        return Err(format!("获取 client 失败: HTTP {} -> {}", status, data));
    }
    Ok(())
}

fn sign_in_with_password(
    client: &reqwest::blocking::Client,
    email: &str,
    password: &str,
    timeout: i64,
) -> Result<Value, String> {
    let url = format!("{CLERK_BASE}/v1/client/sign_ins");
    let form = [
        ("locale", "zh-CN"),
        ("identifier", email),
        ("password", password),
        ("strategy", "password"),
    ];

    let resp = client
        .post(url)
        .query(&clerk_params())
        .header("accept", "*/*")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("origin", ORCHIDS_WEB)
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .form(&form)
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| e.to_string())?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);
    if status >= 400 {
        return Err(format!("sign_in 失败: HTTP {} -> {}", status, data));
    }
    Ok(data)
}

fn touch_session(
    client: &reqwest::blocking::Client,
    session_id: &str,
    timeout: i64,
) -> Result<Value, String> {
    let url = format!("{CLERK_BASE}/v1/client/sessions/{session_id}/touch");
    let resp = client
        .post(url)
        .query(&clerk_params())
        .header("accept", "*/*")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("origin", ORCHIDS_WEB)
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .form(&[("active_organization_id", "")])
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| e.to_string())?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);
    if status >= 400 {
        return Err(format!("touch session 失败: HTTP {} -> {}", status, data));
    }
    Ok(data)
}

fn init_profile_state(
    client: &reqwest::blocking::Client,
    timeout: i64,
    deployment_id: Option<&str>,
) -> Result<(), String> {
    let mut req = client
        .post(format!("{ORCHIDS_WEB}/"))
        .header("accept", "text/x-component")
        .header("content-type", "text/plain;charset=UTF-8")
        .header("origin", ORCHIDS_WEB)
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .header("next-action", NEXT_ACTION_PROFILE_INIT)
        .header("next-router-state-tree", NEXT_ROUTER_STATE_TREE)
        .header("sec-fetch-site", "same-origin")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-dest", "empty")
        .header("accept-language", "zh-CN,zh;q=0.9")
        .body("[\"$undefined\",\"$undefined\"]")
        .timeout(req_timeout_secs(timeout));
    if let Some(did) = deployment_id {
        req = req.header("x-deployment-id", did);
    }
    let resp = req.send().map_err(|e| e.to_string())?;

    let status = resp.status().as_u16();
    if status >= 400 {
        let body = resp.text().unwrap_or_default();
        return Err(format!("初始化 profile state 失败: HTTP {} -> {}", status, body));
    }
    Ok(())
}

fn fetch_profile(
    client: &reqwest::blocking::Client,
    user_id: &str,
    timeout: i64,
    deployment_id: Option<&str>,
) -> Result<ProfileResult, String> {
    let body = format!("[\"{}\"]", user_id);
    let mut req = client
        .post(format!("{ORCHIDS_WEB}/"))
        .header("accept", "text/x-component")
        .header("content-type", "text/plain;charset=UTF-8")
        .header("origin", ORCHIDS_WEB)
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .header("next-action", NEXT_ACTION_GET_USER)
        .header("next-router-state-tree", NEXT_ROUTER_STATE_TREE)
        .header("sec-fetch-site", "same-origin")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-dest", "empty")
        .header("accept-language", "zh-CN,zh;q=0.9")
        .body(body)
        .timeout(req_timeout_secs(timeout));
    if let Some(did) = deployment_id {
        req = req.header("x-deployment-id", did);
    }

    let resp = req.send().map_err(|e| e.to_string())?;

    let status = resp.status().as_u16();
    let raw = resp.text().unwrap_or_default();
    if status >= 400 {
        return Err(format!("获取 profile 失败: HTTP {} -> {}", status, raw));
    }

    let objs = extract_json_objects_from_rsc_text(&raw);
    for obj in objs {
        let plan = obj.get("plan").and_then(Value::as_str).map(str::to_string);
        let credits = obj.get("credits").and_then(Value::as_i64);
        let user = obj.get("userId").and_then(Value::as_str);
        if user.is_some() && (plan.is_some() || credits.is_some()) {
            return Ok(ProfileResult { plan, credits });
        }
    }

    Err(format!("响应中未找到 profile/credits 字段: {}", raw))
}

fn get_home_deployment_id(client: &reqwest::blocking::Client, timeout: i64) -> Option<String> {
    let resp = client
        .get(format!("{ORCHIDS_WEB}/"))
        .header("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("user-agent", user_agent())
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .timeout(req_timeout_secs(timeout))
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.headers()
        .get("x-deployment-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

fn extract_created_session_id(payload: &Value) -> Option<String> {
    payload
        .get("response")
        .and_then(Value::as_object)
        .and_then(|o| o.get("created_session_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_user_id_from_touch(payload: &Value) -> Option<String> {
    payload
        .get("response")
        .and_then(Value::as_object)
        .and_then(|o| o.get("user"))
        .and_then(Value::as_object)
        .and_then(|o| o.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_json_objects_from_rsc_text(payload: &str) -> Vec<Value> {
    let mut objs = Vec::new();
    for line in payload.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let candidate = if trimmed
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            trimmed
                .split_once(':')
                .map(|(_, rest)| rest.trim())
                .unwrap_or(trimmed)
        } else {
            trimmed
        };

        if !candidate.starts_with('{') {
            continue;
        }

        if let Ok(v) = serde_json::from_str::<Value>(candidate) {
            objs.push(v);
        }
    }
    objs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_fetch_plan_and_credits_from_env() {
        let email = match std::env::var("ORCHIDS_TEST_EMAIL") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => {
                eprintln!("skip live test: ORCHIDS_TEST_EMAIL 未设置");
                return;
            }
        };
        let password = match std::env::var("ORCHIDS_TEST_PASSWORD") {
            Ok(v) if !v.trim().is_empty() => v,
            _ => {
                eprintln!("skip live test: ORCHIDS_TEST_PASSWORD 未设置");
                return;
            }
        };

        let result = fetch_plan_and_credits(&email, &password, 30, None)
            .unwrap_or_else(|e| panic!("live fetch failed: {}", e));

        eprintln!(
            "live fetch ok: email={}, plan={:?}, credits={:?}",
            email, result.plan, result.credits
        );

        assert!(result.plan.is_some(), "plan 为空");
        assert!(result.credits.is_some(), "credits 为空");
    }
}
