use std::sync::Arc;
use std::time::Duration;

use cookie_store::CookieStore;
use reqwest::blocking::{Client, ClientBuilder, Response};
use reqwest::header::USER_AGENT;
use reqwest_cookie_store::CookieStoreMutex;
use serde_json::{json, Value};

const WORKER_SAFE_USER_AGENT: &str = "reqwest/0.12.24";

use crate::errors::AppError;

pub type SharedCookieStore = Arc<CookieStoreMutex>;

pub fn create_client(proxy: Option<&str>) -> Result<(Client, SharedCookieStore), AppError> {
    let cookie_store = Arc::new(CookieStoreMutex::new(CookieStore::default()));
    let mut builder = ClientBuilder::new()
        .cookie_provider(cookie_store.clone())
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(USER_AGENT, reqwest::header::HeaderValue::from_static(WORKER_SAFE_USER_AGENT));
            headers
        });
    if let Some(proxy_url) = proxy {
        if !proxy_url.is_empty() {
            let p = reqwest::Proxy::all(proxy_url)
                .map_err(|e| AppError::Runtime(format!("无效的代理地址: {}", e)))?
                .no_proxy(reqwest::NoProxy::from_string("localhost,127.0.0.1"));
            builder = builder.proxy(p);
        }
    }
    let client = builder
        .build()
        .map_err(|e| AppError::Runtime(e.to_string()))?;
    Ok((client, cookie_store))
}

pub fn req_timeout_secs(timeout: i64) -> Duration {
    Duration::from_secs(timeout.max(0) as u64)
}

pub fn json_or_raw(resp: Response) -> Value {
    let text = match resp.text() {
        Ok(t) => t,
        Err(_) => String::new(),
    };

    match serde_json::from_str::<Value>(&text) {
        Ok(v) => v,
        Err(_) => json!({"raw": text}),
    }
}

pub fn json_compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
}
