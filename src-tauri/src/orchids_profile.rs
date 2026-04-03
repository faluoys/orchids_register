use std::collections::HashSet;
use std::fmt;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use regex::Regex;
use serde_json::Value;

use orchids_core::constants::user_agent;
use orchids_core::http_client::{create_client, json_or_raw, req_timeout_secs};

const CLERK_BASE: &str = "https://clerk.orchids.app";
const ORCHIDS_WEB: &str = "https://www.orchids.app";
const API_VERSION: &str = "2025-11-10";
const JS_VERSION: &str = "5.125.3";
const DEFAULT_PROFILE_ACTION_ID: &str = "4024929f98f58f3e813cf1e5f42d2e952b1dde0f40";
const ACTION_CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const DEFAULT_NEXT_ROUTER_STATE_TREE: &str =
    r#"["",{"children":["__PAGE__",{},null,null]},null,null,true]"#;
const ACTION_NAME_HINTS: [&str; 6] = [
    "getUserProfile",
    "getUserCredits",
    "getCredits",
    "getUserUsage",
    "getUsage",
    "getUser",
];

#[derive(Debug, Clone)]
pub struct ProfileResult {
    pub plan: Option<String>,
    pub credits: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExistingSessionContext {
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub session_jwt: Option<String>,
}

#[derive(Debug, Clone)]
struct ActionCacheEntry {
    id: String,
    fetched_at: Instant,
}

#[derive(Debug)]
enum ProfileFetchError {
    ActionNotFound(String),
    Other(String),
}

impl fmt::Display for ProfileFetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActionNotFound(message) | Self::Other(message) => f.write_str(message),
        }
    }
}

#[allow(dead_code)]
pub fn fetch_plan_and_credits(
    email: &str,
    password: &str,
    timeout: i64,
    proxy: Option<&str>,
) -> Result<ProfileResult, String> {
    fetch_plan_and_credits_with_session(email, password, timeout, proxy, None)
}

pub fn fetch_plan_and_credits_with_session(
    email: &str,
    password: &str,
    timeout: i64,
    proxy: Option<&str>,
    existing_session: Option<&ExistingSessionContext>,
) -> Result<ProfileResult, String> {
    let (client, _) = create_client(proxy).map_err(|e| e.to_string())?;

    if let Some(existing_session) = normalize_existing_session_context(existing_session) {
        eprintln!(
            "[profile] 尝试复用已有会话: {}",
            describe_existing_session_context(&existing_session)
        );
        match fetch_profile_from_existing_session(&client, &existing_session, timeout) {
            Ok(profile) => {
                eprintln!("[profile] 复用已有会话获取 profile 成功");
                return Ok(profile);
            }
            Err(err) => {
                eprintln!("[profile] 复用已有会话失败，回退密码登录: {}", err);
            }
        }
    }

    eprintln!("[profile] 使用密码登录链路获取 profile");
    init_environment(&client, timeout)?;
    get_client(&client, timeout)?;
    let sign_in = sign_in_with_password(&client, email, password, timeout)?;
    let session_id = extract_created_session_id(&sign_in)
        .ok_or_else(|| "登录成功但未获取到 created_session_id".to_string())?;
    let touch_data = touch_session(&client, &session_id, timeout)?;
    let user_id = extract_user_id_from_touch(&touch_data)
        .ok_or_else(|| "touch 成功但未获取到 user_id".to_string())?;
    let session_jwt = fetch_session_jwt(&client, &session_id, timeout)?;

    fetch_profile(&client, &session_jwt, &user_id, timeout)
}

fn normalize_existing_session_context(
    existing_session: Option<&ExistingSessionContext>,
) -> Option<ExistingSessionContext> {
    let existing_session = existing_session?;
    let session_id = normalize_optional_text(existing_session.session_id.as_deref());
    let user_id = normalize_optional_text(existing_session.user_id.as_deref());
    let session_jwt = normalize_optional_text(existing_session.session_jwt.as_deref());

    let has_way_to_get_jwt = session_jwt.is_some() || session_id.is_some();
    let has_way_to_get_user = user_id.is_some() || session_id.is_some();
    if has_way_to_get_jwt && has_way_to_get_user {
        Some(ExistingSessionContext {
            session_id,
            user_id,
            session_jwt,
        })
    } else {
        None
    }
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn describe_existing_session_context(existing_session: &ExistingSessionContext) -> String {
    format!(
        "session_id={}, user_id={}, jwt={}",
        existing_session.session_id.as_ref().map(|_| "yes").unwrap_or("no"),
        existing_session.user_id.as_ref().map(|_| "yes").unwrap_or("no"),
        existing_session.session_jwt.as_ref().map(|_| "yes").unwrap_or("no"),
    )
}

fn fetch_profile_from_existing_session(
    client: &reqwest::blocking::Client,
    existing_session: &ExistingSessionContext,
    timeout: i64,
) -> Result<ProfileResult, String> {
    let session_id = existing_session.session_id.as_deref();
    let user_id = match existing_session.user_id.clone() {
        Some(user_id) => user_id,
        None => {
            let session_id = session_id
                .ok_or_else(|| "复用已有会话失败: 缺少 session_id，无法恢复 user_id".to_string())?;
            let touch_data = touch_session(client, session_id, timeout)?;
            extract_user_id_from_touch(&touch_data)
                .ok_or_else(|| "复用已有会话失败: touch session 成功但未拿到 user_id".to_string())?
        }
    };

    if let Some(session_jwt) = existing_session.session_jwt.as_deref() {
        match fetch_profile(client, session_jwt, &user_id, timeout) {
            Ok(profile) => return Ok(profile),
            Err(err) => {
                if let Some(session_id) = session_id {
                    let fresh_session_jwt = fetch_session_jwt(client, session_id, timeout)
                        .map_err(|refresh_err| format!("复用已有会话失败: {}; 刷新 session jwt 失败: {}", err, refresh_err))?;
                    return fetch_profile(client, &fresh_session_jwt, &user_id, timeout)
                        .map_err(|refresh_err| format!("复用已有会话失败: {}; 使用 session_id 刷新 jwt 后仍失败: {}", err, refresh_err));
                }
                return Err(format!("复用已有会话失败: {}", err));
            }
        }
    }

    let session_id =
        session_id.ok_or_else(|| "复用已有会话失败: 缺少 session_id，无法换取 session jwt".to_string())?;
    let session_jwt = fetch_session_jwt(client, session_id, timeout)?;
    fetch_profile(client, &session_jwt, &user_id, timeout)
        .map_err(|err| format!("复用已有会话失败: {}", err))
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

fn fetch_session_jwt(
    client: &reqwest::blocking::Client,
    session_id: &str,
    timeout: i64,
) -> Result<String, String> {
    let url = format!("{CLERK_BASE}/v1/client/sessions/{session_id}/tokens");
    let resp = client
        .post(url)
        .query(&[
            ("__clerk_api_version", API_VERSION),
            ("_clerk_js_version", JS_VERSION),
            ("debug", "skip_cache"),
        ])
        .header("accept", "*/*")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("origin", ORCHIDS_WEB)
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .header("accept-language", "zh-CN,zh;q=0.9")
        .form(&[("organization_id", "")])
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| e.to_string())?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);
    if status >= 400 {
        return Err(format!("获取 session jwt 失败: HTTP {} -> {}", status, data));
    }

    find_first_jwt(&data).ok_or_else(|| format!("tokens 响应里未找到 jwt: {}", data))
}

fn fetch_profile(
    client: &reqwest::blocking::Client,
    session_jwt: &str,
    user_id: &str,
    timeout: i64,
) -> Result<ProfileResult, String> {
    let mut action_id = resolve_profile_action_id(client, timeout);

    let mut last_err = match fetch_profile_with_action_enhanced(client, session_jwt, user_id, &action_id, timeout, false) {
        Ok(profile) => return Ok(profile),
        Err(ProfileFetchError::ActionNotFound(_)) => {
            invalidate_profile_action_cache();
            action_id = resolve_profile_action_id(client, timeout);
            match fetch_profile_with_action_enhanced(client, session_jwt, user_id, &action_id, timeout, false) {
                Ok(profile) => return Ok(profile),
                Err(err) => err,
            }
        }
        Err(err) => err,
    };

    if profile_error_needs_context_retry(&last_err) {
        match fetch_profile_with_action_enhanced(client, session_jwt, user_id, &action_id, timeout, true) {
            Ok(profile) => return Ok(profile),
            Err(err) => last_err = err,
        }
    }

    Err(last_err.to_string())
}

#[allow(dead_code)]
fn fetch_profile_with_action(
    client: &reqwest::blocking::Client,
    session_jwt: &str,
    user_id: &str,
    action_id: &str,
    timeout: i64,
    include_context: bool,
) -> Result<ProfileResult, ProfileFetchError> {
    let body = format!(r#"["{}"]"#, user_id);
    let mut req = client
        .post(format!("{ORCHIDS_WEB}/"))
        .header("accept", "text/x-component")
        .header("content-type", "text/plain;charset=UTF-8")
        .header("origin", ORCHIDS_WEB)
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .header("next-action", action_id)
        .header("accept-language", "zh-CN,zh;q=0.9")
        .header("sec-fetch-site", "same-origin")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-dest", "empty")
        .header("cookie", format!("__session={session_jwt}"))
        .body(body);

    if include_context {
        let state_tree = fetch_next_router_state_tree(client, timeout)
            .unwrap_or_else(|| DEFAULT_NEXT_ROUTER_STATE_TREE.to_string());
        req = req.header("next-router-state-tree", state_tree);

        if let Some(deployment_id) = get_home_deployment_id(client, timeout) {
            req = req.header("x-deployment-id", deployment_id);
        }
    }

    let resp = req
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| ProfileFetchError::Other(e.to_string()))?;

    let status = resp.status().as_u16();
    let action_not_found = resp
        .headers()
        .get("x-nextjs-action-not-found")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == "1")
        .unwrap_or(false);
    let raw = resp.text().unwrap_or_default();

    if status >= 400 {
        let message = format!("获取 profile 失败: HTTP {} -> {}", status, raw);
        if action_not_found || raw.to_ascii_lowercase().contains("server action not found") {
            return Err(ProfileFetchError::ActionNotFound(message));
        }
        return Err(ProfileFetchError::Other(message));
    }

    parse_profile_from_rsc_text(&raw)
        .ok_or_else(|| ProfileFetchError::Other(format!("响应中未找到 profile/credits 字段: {}", raw)))
}

fn fetch_profile_with_action_enhanced(
    client: &reqwest::blocking::Client,
    session_jwt: &str,
    user_id: &str,
    action_id: &str,
    timeout: i64,
    include_context: bool,
) -> Result<ProfileResult, ProfileFetchError> {
    let body = format!(r#"["{}"]"#, user_id);
    let mut req = client
        .post(format!("{ORCHIDS_WEB}/"))
        .header("accept", "text/x-component")
        .header("content-type", "text/plain;charset=UTF-8")
        .header("origin", ORCHIDS_WEB)
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .header("next-action", action_id)
        .header("accept-language", "zh-CN,zh;q=0.9")
        .header("sec-fetch-site", "same-origin")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-dest", "empty")
        .header("cookie", format!("__session={session_jwt}"))
        .body(body);

    if include_context {
        let state_tree = fetch_next_router_state_tree(client, timeout)
            .unwrap_or_else(|| DEFAULT_NEXT_ROUTER_STATE_TREE.to_string());
        req = req.header("next-router-state-tree", state_tree);

        if let Some(deployment_id) = get_home_deployment_id(client, timeout) {
            req = req.header("x-deployment-id", deployment_id);
        }
    }

    let resp = req
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| ProfileFetchError::Other(e.to_string()))?;

    let status = resp.status().as_u16();
    let action_not_found = resp
        .headers()
        .get("x-nextjs-action-not-found")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == "1")
        .unwrap_or(false);
    let raw = resp.text().unwrap_or_default();

    if status >= 400 {
        let message = profile_http_error_message(status, &raw);
        if action_not_found || raw.to_ascii_lowercase().contains("server action not found") {
            return Err(ProfileFetchError::ActionNotFound(message));
        }
        return Err(ProfileFetchError::Other(message));
    }

    parse_profile_from_rsc_text(&raw)
        .ok_or_else(|| ProfileFetchError::Other(format!("鍝嶅簲涓湭鎵惧埌 profile/credits 瀛楁: {}", raw)))
}

fn profile_error_needs_context_retry(err: &ProfileFetchError) -> bool {
    match err {
        ProfileFetchError::ActionNotFound(_) => true,
        ProfileFetchError::Other(message) => {
            profile_response_needs_context_retry(0, message)
                || message.contains("响应中未找到 profile/credits 字段")
        }
    }
}

fn resolve_profile_action_id(client: &reqwest::blocking::Client, timeout: i64) -> String {
    if let Ok(cache) = profile_action_cache().lock() {
        if let Some(entry) = cache.as_ref() {
            if entry.fetched_at.elapsed() < ACTION_CACHE_TTL {
                return entry.id.clone();
            }
        }
    }

    let action_id = discover_profile_action_id(client, timeout)
        .unwrap_or_else(|_| DEFAULT_PROFILE_ACTION_ID.to_string());

    if let Ok(mut cache) = profile_action_cache().lock() {
        *cache = Some(ActionCacheEntry {
            id: action_id.clone(),
            fetched_at: Instant::now(),
        });
    }

    action_id
}

fn invalidate_profile_action_cache() {
    if let Ok(mut cache) = profile_action_cache().lock() {
        *cache = None;
    }
}

fn profile_action_cache() -> &'static Mutex<Option<ActionCacheEntry>> {
    static CACHE: OnceLock<Mutex<Option<ActionCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn discover_profile_action_id(
    client: &reqwest::blocking::Client,
    timeout: i64,
) -> Result<String, String> {
    let html = fetch_text(client, ORCHIDS_WEB, timeout)?;
    let script_urls = extract_script_urls_from_html(&html);
    if script_urls.is_empty() {
        return Err("首页未找到 Orchids JS chunk".to_string());
    }

    let mut fuzzy_match = None;
    for url in script_urls {
        if !url.contains("/_next/static/chunks/") {
            continue;
        }

        let js = match fetch_text(client, &url, timeout) {
            Ok(content) => content,
            Err(_) => continue,
        };

        for (id, name) in extract_server_actions_from_js(&js) {
            if ACTION_NAME_HINTS.iter().any(|hint| *hint == name) {
                return Ok(id);
            }

            let lower = name.to_ascii_lowercase();
            if fuzzy_match.is_none()
                && (lower.contains("profile")
                    || lower.contains("credit")
                    || lower.contains("usage")
                    || lower.contains("quota"))
            {
                fuzzy_match = Some(id);
            }
        }
    }

    fuzzy_match.ok_or_else(|| "未发现 Orchids profile action".to_string())
}

fn fetch_text(
    client: &reqwest::blocking::Client,
    url: &str,
    timeout: i64,
) -> Result<String, String> {
    let resp = client
        .get(url)
        .header("accept", "*/*")
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| e.to_string())?;

    let status = resp.status().as_u16();
    if status >= 400 {
        let body = resp.text().unwrap_or_default();
        return Err(format!("抓取 {} 失败: HTTP {} -> {}", url, status, body));
    }

    resp.text().map_err(|e| e.to_string())
}

fn fetch_next_router_state_tree(
    client: &reqwest::blocking::Client,
    timeout: i64,
) -> Option<String> {
    let resp = client
        .get(ORCHIDS_WEB)
        .header("rsc", "1")
        .header("next-router-prefetch", "1")
        .header("origin", ORCHIDS_WEB)
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .timeout(req_timeout_secs(timeout))
        .send()
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let payload = resp.text().ok()?;
    extract_router_state_tree_from_rsc_prefetch_payload(&payload)
}

fn extract_router_state_tree_from_rsc_prefetch_payload(payload: &str) -> Option<String> {
    let first_line = payload
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    let json_part = first_line
        .split_once(':')
        .map(|(_, rest)| rest.trim())
        .unwrap_or(first_line);
    let value: Value = serde_json::from_str(json_part).ok()?;
    let tree = find_router_state_tree_value(&value)?;
    serde_json::to_string(tree).ok()
}

fn find_router_state_tree_value(value: &Value) -> Option<&Value> {
    match value {
        Value::Array(items) => {
            let looks_like_tree = items.len() >= 5
                && items.first().map(Value::is_string).unwrap_or(false)
                && items
                    .get(1)
                    .and_then(Value::as_object)
                    .map(|object| object.contains_key("children"))
                    .unwrap_or(false);

            if looks_like_tree {
                return Some(value);
            }

            for item in items {
                if let Some(found) = find_router_state_tree_value(item) {
                    return Some(found);
                }
            }

            None
        }
        Value::Object(map) => {
            for item in map.values() {
                if let Some(found) = find_router_state_tree_value(item) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn get_home_deployment_id(client: &reqwest::blocking::Client, timeout: i64) -> Option<String> {
    let resp = client
        .get(ORCHIDS_WEB)
        .header("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("referer", format!("{ORCHIDS_WEB}/"))
        .header("user-agent", user_agent())
        .timeout(req_timeout_secs(timeout))
        .send()
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    if let Some(value) = resp
        .headers()
        .get("x-deployment-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
    {
        return Some(value);
    }

    let body = resp.text().ok()?;
    Regex::new(r#"dpl_[A-Za-z0-9]+"#)
        .ok()?
        .find(&body)
        .map(|capture| capture.as_str().to_string())
}

fn profile_response_needs_context_retry(status: u16, raw: &str) -> bool {
    if status >= 500 {
        return true;
    }

    let lower = raw.to_ascii_lowercase();
    lower.contains(r#""digest":"#) || lower.contains("\n1:e{")
}

fn profile_response_suggests_visitor_verification(status: u16, raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    (status >= 500 && (lower.contains(r#""digest":"#) || lower.contains("\n1:e{")))
        || lower.contains("visitor")
        || lower.contains("cf-challenge")
        || lower.contains("turnstile")
}

fn profile_http_error_message(status: u16, raw: &str) -> String {
    let mut message = format!("获取 profile 失败: HTTP {} -> {}", status, raw);
    if profile_response_suggests_visitor_verification(status, raw) {
        message.push_str("；疑似触发站点风控/真实访客校验，请先在浏览器完成登录与验证后重试，或优先复用当前账号已保存会话。");
    }
    message
}

fn extract_script_urls_from_html(html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut seen = HashSet::new();

    for caps in script_src_regex().captures_iter(html) {
        let Some(src) = caps.get(1).map(|capture| capture.as_str().trim()) else {
            continue;
        };
        if src.is_empty() {
            continue;
        }

        let absolute = if src.starts_with("http://") || src.starts_with("https://") {
            src.to_string()
        } else if src.starts_with('/') {
            format!("{ORCHIDS_WEB}{src}")
        } else {
            format!("{ORCHIDS_WEB}/{}", src.trim_start_matches("./"))
        };

        if seen.insert(absolute.clone()) {
            urls.push(absolute);
        }
    }

    urls
}

fn extract_server_actions_from_js(js: &str) -> Vec<(String, String)> {
    let mut actions = Vec::new();
    let mut seen = HashSet::new();

    for caps in action_ref_regex().captures_iter(js) {
        let Some(id) = caps.get(1).map(|capture| capture.as_str().trim()) else {
            continue;
        };
        let Some(name) = caps.get(2).map(|capture| capture.as_str().trim()) else {
            continue;
        };
        if id.is_empty() || name.is_empty() {
            continue;
        }

        let item = (id.to_string(), name.to_string());
        if seen.insert(item.clone()) {
            actions.push(item);
        }
    }

    actions
}

fn script_src_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r#"src=["']([^"']+\.js[^"']*)["']"#).unwrap())
}

fn action_ref_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"createServerReference\)\(\s*"([0-9a-f]{40,})"[\s\S]*?"([^"]+)"\s*\)"#)
            .unwrap()
    })
}

fn parse_profile_from_rsc_text(payload: &str) -> Option<ProfileResult> {
    for obj in extract_json_objects_from_rsc_text(payload) {
        if let Some(profile) = find_profile_result_in_value(&obj) {
            return Some(profile);
        }
    }
    None
}

fn find_profile_result_in_value(value: &Value) -> Option<ProfileResult> {
    match value {
        Value::Object(map) => {
            let plan = map
                .get("plan")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let credits = map.get("credits").and_then(value_to_i64);
            let has_user_id = map
                .get("userId")
                .and_then(Value::as_str)
                .or_else(|| map.get("user_id").and_then(Value::as_str))
                .is_some();
            let looks_like_profile = (plan.is_some() || credits.is_some())
                && (has_user_id || (plan.is_some() && credits.is_some()));

            if looks_like_profile {
                return Some(ProfileResult { plan, credits });
            }

            for child in map.values() {
                if let Some(profile) = find_profile_result_in_value(child) {
                    return Some(profile);
                }
            }

            None
        }
        Value::Array(items) => {
            for item in items {
                if let Some(profile) = find_profile_result_in_value(item) {
                    return Some(profile);
                }
            }
            None
        }
        _ => None,
    }
}

fn value_to_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|current| i64::try_from(current).ok()))
}

fn find_first_jwt(payload: &Value) -> Option<String> {
    match payload {
        Value::Object(map) => {
            if let Some(jwt) = map
                .get("jwt")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                return Some(jwt.to_string());
            }

            for value in map.values() {
                if let Some(found) = find_first_jwt(value) {
                    return Some(found);
                }
            }

            None
        }
        Value::Array(items) => {
            for item in items {
                if let Some(found) = find_first_jwt(item) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_created_session_id(payload: &Value) -> Option<String> {
    payload
        .get("response")
        .and_then(Value::as_object)
        .and_then(|object| object.get("created_session_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_user_id_from_touch(payload: &Value) -> Option<String> {
    payload
        .get("response")
        .and_then(Value::as_object)
        .and_then(|object| object.get("user"))
        .and_then(Value::as_object)
        .and_then(|object| object.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_json_objects_from_rsc_text(payload: &str) -> Vec<Value> {
    let mut objs = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in payload.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }

            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    continue;
                }

                depth -= 1;
                if depth == 0 {
                    if let Some(begin) = start.take() {
                        let end = index + ch.len_utf8();
                        if let Ok(value) = serde_json::from_str::<Value>(&payload[begin..end]) {
                            objs.push(value);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    objs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_objects_from_rsc_text_supports_embedded_object_in_array_line() {
        let payload =
            r#"1:["$","$L3",null,{"credits":12345,"plan":"PRO","userId":"user_123"}]"#;

        let objs = extract_json_objects_from_rsc_text(payload);

        assert_eq!(objs.len(), 1, "未提取到嵌入数组里的 profile 对象");
        assert_eq!(objs[0].get("plan").and_then(Value::as_str), Some("PRO"));
        assert_eq!(objs[0].get("credits").and_then(Value::as_i64), Some(12345));
    }

    #[test]
    fn extract_script_urls_from_html_makes_absolute_and_unique() {
        let html = r#"
            <script src="/_next/static/chunks/a.js?dpl=test"></script>
            <script src="https://www.orchids.app/_next/static/chunks/b.js"></script>
            <script src="/_next/static/chunks/a.js?dpl=test"></script>
        "#;

        let urls = extract_script_urls_from_html(html);

        assert_eq!(
            urls,
            vec![
                "https://www.orchids.app/_next/static/chunks/a.js?dpl=test".to_string(),
                "https://www.orchids.app/_next/static/chunks/b.js".to_string(),
            ]
        );
    }

    #[test]
    fn extract_server_actions_from_js_reads_create_server_reference_calls() {
        let js = r#"
            let a=(0,n.createServerReference)(
                "4024929f98f58f3e813cf1e5f42d2e952b1dde0f40",
                n.callServer,
                void 0,
                n.findSourceMapURL,
                "getUserProfile"
            );
        "#;

        let actions = extract_server_actions_from_js(js);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].0, "4024929f98f58f3e813cf1e5f42d2e952b1dde0f40");
        assert_eq!(actions[0].1, "getUserProfile");
    }

    #[test]
    fn profile_response_needs_context_retry_on_digest_500() {
        let raw = r#"0:{"a":"$@1","f":"","b":"CGcit38jVgcutHMlYO4Mb","q":"","i":false}
1:E{"digest":"1288858246"}"#;

        assert!(profile_response_needs_context_retry(500, raw));
    }

    #[test]
    fn profile_http_error_message_marks_possible_visitor_verification() {
        let raw = r#"0:{"a":"$@1","f":"","b":"CGcit38jVgcutHMlYO4Mb","q":"","i":false}
1:E{"digest":"1288858246"}"#;

        let message = profile_http_error_message(500, raw);

        assert!(message.contains("疑似触发站点风控"));
        assert!(message.contains("真实访客"));
    }

    #[test]
    fn describe_existing_session_context_reports_available_fields() {
        let context = ExistingSessionContext {
            session_id: Some("sess_123".to_string()),
            user_id: None,
            session_jwt: Some("jwt_123".to_string()),
        };

        assert_eq!(
            describe_existing_session_context(&context),
            "session_id=yes, user_id=no, jwt=yes"
        );
    }

    #[test]
    fn extract_router_state_tree_from_rsc_prefetch_payload_reads_first_tree() {
        let raw = r#"0:{"f":[[[["",{"children":["__PAGE__",{},null,null]},null,null,true],null,null,true]]]}"#;

        let tree = extract_router_state_tree_from_rsc_prefetch_payload(raw)
            .expect("应当能提取 router state tree");

        assert_eq!(tree, r#"["",{"children":["__PAGE__",{},null,null]},null,null,true]"#);
    }

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

        let result =
            fetch_plan_and_credits(&email, &password, 30, None).unwrap_or_else(|e| panic!("live fetch failed: {}", e));

        eprintln!(
            "live fetch ok: email={}, plan={:?}, credits={:?}",
            email, result.plan, result.credits
        );

        assert!(result.plan.is_some(), "plan 为空");
        assert!(result.credits.is_some(), "credits 为空");
    }
}
