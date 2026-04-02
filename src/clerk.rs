use serde_json::{Map, Value};

use crate::constants::{clerk_params, user_agent, CLERK_BASE};
use crate::errors::AppError;
use crate::http_client::{json_compact, json_or_raw, req_timeout_secs, SharedCookieStore};
use reqwest::blocking::Client;
use url::Url;

pub fn extract_signup_id(payload: &Value) -> Option<String> {
    let direct = payload.get("id").and_then(Value::as_str);
    if let Some(id) = direct {
        if id.starts_with("sua_") {
            return Some(id.to_string());
        }
    }

    if let Some(sid) = payload
        .get("sign_up")
        .and_then(Value::as_object)
        .and_then(|o| o.get("id"))
        .and_then(Value::as_str)
    {
        return Some(sid.to_string());
    }

    if let Some(sid) = payload
        .get("response")
        .and_then(Value::as_object)
        .and_then(|o| o.get("id"))
        .and_then(Value::as_str)
    {
        return Some(sid.to_string());
    }

    if let Some(sid) = payload
        .get("meta")
        .and_then(Value::as_object)
        .and_then(|m| m.get("client"))
        .and_then(Value::as_object)
        .and_then(|c| c.get("sign_up"))
        .and_then(Value::as_object)
        .and_then(|s| s.get("id"))
        .and_then(Value::as_str)
    {
        return Some(sid.to_string());
    }

    None
}

pub fn init_clerk_environment(client: &Client, timeout: i64) -> Result<Value, AppError> {
    let url = format!("{CLERK_BASE}/v1/environment");
    let resp = client
        .get(url)
        .query(&clerk_params())
        .header("accept", "*/*")
        .header("origin", "https://www.orchids.app")
        .header("referer", "https://www.orchids.app/")
        .header("user-agent", user_agent())
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| AppError::Runtime(e.to_string()))?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);
    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "初始化环境失败: HTTP {} -> {}",
            status,
            json_compact(&data)
        )));
    }
    Ok(data)
}

pub fn create_sign_up(
    client: &Client,
    email: &str,
    password: &str,
    captcha_token: &str,
    locale: &str,
    timeout: i64,
) -> Result<Value, AppError> {
    let url = format!("{CLERK_BASE}/v1/client/sign_ups");
    let form = [
        ("email_address", email),
        ("password", password),
        ("locale", locale),
        ("captcha_token", captcha_token),
        ("captcha_widget_type", "smart"),
    ];

    let resp = client
        .post(url)
        .query(&clerk_params())
        .header("accept", "*/*")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("origin", "https://accounts.orchids.app")
        .header("referer", "https://accounts.orchids.app/")
        .header("user-agent", user_agent())
        .form(&form)
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| AppError::Runtime(e.to_string()))?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);
    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "创建 sign_up 失败: HTTP {} -> {}",
            status,
            json_compact(&data)
        )));
    }
    Ok(data)
}

pub fn prepare_email_verification(client: &Client, sign_up_id: &str, timeout: i64) -> Result<Value, AppError> {
    let url = format!("{CLERK_BASE}/v1/client/sign_ups/{sign_up_id}/prepare_verification");
    let form = [("strategy", "email_code")];
    let resp = client
        .post(url)
        .query(&clerk_params())
        .header("accept", "*/*")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("origin", "https://accounts.orchids.app")
        .header("referer", "https://accounts.orchids.app/")
        .header("user-agent", user_agent())
        .form(&form)
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| AppError::Runtime(e.to_string()))?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);
    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "prepare_verification 失败: HTTP {} -> {}",
            status,
            json_compact(&data)
        )));
    }
    Ok(data)
}

pub fn attempt_email_verification(
    client: &Client,
    sign_up_id: &str,
    email_code: &str,
    timeout: i64,
) -> Result<Value, AppError> {
    let url = format!("{CLERK_BASE}/v1/client/sign_ups/{sign_up_id}/attempt_verification");
    let form = [("code", email_code), ("strategy", "email_code")];
    let resp = client
        .post(url)
        .query(&clerk_params())
        .header("accept", "*/*")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("origin", "https://accounts.orchids.app")
        .header("referer", "https://accounts.orchids.app/")
        .header("user-agent", user_agent())
        .form(&form)
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| AppError::Runtime(e.to_string()))?;

    let status = resp.status().as_u16();
    let data = json_or_raw(resp);
    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "attempt_verification 失败: HTTP {} -> {}",
            status,
            json_compact(&data)
        )));
    }
    Ok(data)
}

pub fn extract_client_cookie(cookie_store: &SharedCookieStore) -> Option<String> {
    let urls = [
        Url::parse("https://clerk.orchids.app/").ok(),
        Url::parse("https://accounts.orchids.app/").ok(),
        Url::parse("https://www.orchids.app/").ok(),
    ];

    let Ok(store) = cookie_store.lock() else {
        return None;
    };

    for url in urls.into_iter().flatten() {
        for (name, value) in store.get_request_values(&url) {
            if name == "__client" && !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    None
}

pub fn pick_summary_fields(payload: &Value) -> Value {
    let mut m = Map::new();
    for key in ["id", "status", "object"] {
        m.insert(key.to_string(), payload.get(key).cloned().unwrap_or(Value::Null));
    }
    Value::Object(m)
}
