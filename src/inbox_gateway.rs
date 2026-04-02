use std::collections::HashMap;

use reqwest::blocking::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};

use crate::cli::Args;
use crate::errors::AppError;
use crate::http_client::{json_compact, json_or_raw, req_timeout_secs};

const GATEWAY_POLL_TIMEOUT_BUFFER_SECS: i64 = 5;

#[derive(Debug, Clone)]
pub struct GatewaySettings {
    pub mode: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub provider: String,
    pub provider_mode: String,
    pub project_code: Option<String>,
    pub domain: Option<String>,
}

impl GatewaySettings {
    pub fn from_args(args: &Args) -> Self {
        Self {
            mode: args.mail_mode.clone(),
            base_url: args.mail_gateway_base_url.clone().unwrap_or_default(),
            api_key: args.mail_gateway_api_key.clone(),
            provider: args.mail_provider.clone(),
            provider_mode: args.mail_provider_mode.clone(),
            project_code: args.mail_project_code.clone(),
            domain: args.mail_domain.clone(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.mode.eq_ignore_ascii_case("gateway")
            && !self.base_url.trim().is_empty()
            && !self.provider.trim().is_empty()
    }

    pub fn validate(&self) -> Result<(), AppError> {
        if !self.mode.eq_ignore_ascii_case("gateway") {
            return Ok(());
        }

        if self.base_url.trim().is_empty() {
            return Err(AppError::Usage(
                "mail_mode=gateway 时必须提供 --mail-gateway-base-url".to_string(),
            ));
        }
        if self.provider.trim().is_empty() {
            return Err(AppError::Usage(
                "mail_mode=gateway 时必须提供 --mail-provider".to_string(),
            ));
        }
        if self.provider_mode.trim().is_empty() {
            return Err(AppError::Usage(
                "mail_mode=gateway 时必须提供 --mail-provider-mode".to_string(),
            ));
        }

        Ok(())
    }

    fn acquire_url(&self) -> String {
        format!("{}/v1/inboxes/acquire", self.base_url.trim_end_matches('/'))
    }

    fn poll_url(&self, session_id: &str) -> String {
        format!(
            "{}/v1/inboxes/{}/poll-code",
            self.base_url.trim_end_matches('/'),
            session_id
        )
    }

    fn release_url(&self, session_id: &str) -> String {
        format!("{}/v1/inboxes/{}", self.base_url.trim_end_matches('/'), session_id)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AcquireInboxRequest {
    pub provider: String,
    pub mode: String,
    pub project: Option<String>,
    pub domain: Option<String>,
    pub prefix: Option<String>,
    pub quantity: i32,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AcquireInboxResponse {
    pub session_id: String,
    pub address: String,
    pub provider: String,
    pub mode: String,
    pub expires_at: Option<String>,
    pub upstream_ref: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PollCodeRequest {
    pub timeout_seconds: i64,
    pub interval_seconds: f64,
    pub code_pattern: String,
    pub after_ts: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PollCodeResponse {
    pub status: String,
    pub code: Option<String>,
    pub message_id: Option<String>,
    pub received_at: Option<String>,
    #[serde(default)]
    pub summary: HashMap<String, String>,
}

pub fn gateway_poll_http_timeout_secs(timeout: i64, poll_timeout: i64) -> i64 {
    timeout.max(poll_timeout.saturating_add(GATEWAY_POLL_TIMEOUT_BUFFER_SECS))
}

pub fn acquire_inbox(
    client: &Client,
    timeout: i64,
    settings: &GatewaySettings,
) -> Result<AcquireInboxResponse, AppError> {
    let response = with_gateway_api_key(
        client.post(settings.acquire_url()).timeout(req_timeout_secs(timeout)),
        settings.api_key.as_deref(),
    )
    .json(&AcquireInboxRequest {
        provider: settings.provider.clone(),
        mode: settings.provider_mode.clone(),
        project: settings.project_code.clone(),
        domain: settings.domain.clone(),
        prefix: None,
        quantity: 1,
        metadata: HashMap::new(),
    })
    .send()
    .map_err(|e| AppError::Runtime(format!("mail-gateway acquire 请求失败: {}", e)))?;

    let status = response.status().as_u16();
    let payload = json_or_raw(response);
    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "mail-gateway acquire 失败: HTTP {} -> {}",
            status,
            json_compact(&payload)
        )));
    }

    serde_json::from_value(payload)
        .map_err(|e| AppError::Runtime(format!("mail-gateway acquire 响应解析失败: {}", e)))
}

pub fn poll_code(
    client: &Client,
    timeout: i64,
    settings: &GatewaySettings,
    session_id: &str,
    request: &PollCodeRequest,
) -> Result<PollCodeResponse, AppError> {
    let response = with_gateway_api_key(
        client
            .post(settings.poll_url(session_id))
            .timeout(req_timeout_secs(gateway_poll_http_timeout_secs(timeout, request.timeout_seconds))),
        settings.api_key.as_deref(),
    )
    .json(request)
    .send()
    .map_err(|e| AppError::Runtime(format!("mail-gateway poll-code 请求失败: {}", e)))?;

    let status = response.status().as_u16();
    let payload = json_or_raw(response);
    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "mail-gateway poll-code 失败: HTTP {} -> {}",
            status,
            json_compact(&payload)
        )));
    }

    serde_json::from_value(payload)
        .map_err(|e| AppError::Runtime(format!("mail-gateway poll-code 响应解析失败: {}", e)))
}

pub fn release_inbox(
    client: &Client,
    timeout: i64,
    settings: &GatewaySettings,
    session_id: &str,
) -> Result<(), AppError> {
    let response = with_gateway_api_key(
        client
            .delete(settings.release_url(session_id))
            .timeout(req_timeout_secs(timeout)),
        settings.api_key.as_deref(),
    )
    .send()
    .map_err(|e| AppError::Runtime(format!("mail-gateway release 请求失败: {}", e)))?;

    let status = response.status().as_u16();
    if status == 204 || status == 404 {
        return Ok(());
    }

    let payload = json_or_raw(response);
    Err(AppError::Runtime(format!(
        "mail-gateway release 失败: HTTP {} -> {}",
        status,
        json_compact(&payload)
    )))
}

fn with_gateway_api_key(builder: RequestBuilder, api_key: Option<&str>) -> RequestBuilder {
    if let Some(token) = api_key.filter(|value| !value.trim().is_empty()) {
        builder.header("X-API-Key", token.trim())
    } else {
        builder
    }
}
