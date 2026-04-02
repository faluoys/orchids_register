use reqwest::blocking::Client;
use serde_json::{json, Value};

use crate::constants::{API_VERSION, CLERK_BASE};
use crate::errors::AppError;
use crate::http_client::{json_or_raw, req_timeout_secs};

fn find_first_jwt(payload: &Value) -> Option<String> {
    match payload {
        Value::Object(map) => {
            if let Some(jwt) = map.get("jwt").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                return Some(jwt.to_string());
            }
            for v in map.values() {
                if let Some(found) = find_first_jwt(v) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(arr) => {
            for item in arr {
                if let Some(found) = find_first_jwt(item) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

pub fn test_desktop_session(client: &Client, session_id: &str, timeout: i64) -> Result<Value, AppError> {
    let desktop_js_version = "5.117.0";
    let params = [
        ("__clerk_api_version", API_VERSION),
        ("_clerk_js_version", desktop_js_version),
    ];

    let touch_url = format!("{CLERK_BASE}/v1/client/sessions/{session_id}/touch");
    let touch_resp = client
        .post(touch_url)
        .query(&params)
        .header("accept", "*/*")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Orchids/1.0.6 Chrome/138.0.7204.251 Electron/37.10.3 Safari/537.36")
        .header("accept-language", "zh-CN")
        .body("")
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| AppError::Runtime(e.to_string()))?;

    let touch_status = touch_resp.status().as_u16();
    let touch_data = json_or_raw(touch_resp);

    let token_url = format!("{CLERK_BASE}/v1/client/sessions/{session_id}/tokens");
    let token_resp = client
        .post(token_url)
        .query(&[
            ("__clerk_api_version", API_VERSION),
            ("_clerk_js_version", desktop_js_version),
            ("debug", "skip_cache"),
        ])
        .header("accept", "*/*")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Orchids/1.0.6 Chrome/138.0.7204.251 Electron/37.10.3 Safari/537.36")
        .header("accept-language", "zh-CN")
        .form(&[("organization_id", "")])
        .timeout(req_timeout_secs(timeout))
        .send()
        .map_err(|e| AppError::Runtime(e.to_string()))?;

    let token_status = token_resp.status().as_u16();
    let token_data = json_or_raw(token_resp);
    let jwt = find_first_jwt(&token_data);

    Ok(json!({
        "touch_http_status": touch_status,
        "touch_ok": touch_status < 400,
        "tokens_http_status": token_status,
        "tokens_ok": token_status < 400,
        "jwt": jwt,
        "session_usable_for_desktop": (touch_status < 400) && (token_status < 400) && jwt.is_some(),
        "touch_response": touch_data,
        "tokens_response": token_data,
    }))
}
