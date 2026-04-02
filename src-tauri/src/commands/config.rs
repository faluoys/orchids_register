use std::collections::HashMap;

use serde::Serialize;
use tauri::State;

use crate::db;
use crate::state::AppState;

use orchids_core::http_client::create_client;

#[tauri::command]
pub async fn get_all_config(
    state: State<'_, AppState>,
) -> Result<HashMap<String, String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_all_config(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_config(
    state: State<'_, AppState>,
    configs: HashMap<String, String>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    for (key, value) in &configs {
        db::save_config(&conn, key, value).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn reset_config(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::reset_config(&conn).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxyTestResult {
    pub ip: String,
    pub country: String,
    pub city: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MailGatewayHealthResult {
    pub status: String,
    pub timestamp: i64,
    pub providers: HashMap<String, String>,
}

#[tauri::command]
pub async fn test_proxy(
    state: State<'_, AppState>,
) -> Result<ProxyTestResult, String> {
    let proxy: Option<String> = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::get_all_config(&conn)
            .ok()
            .and_then(|c| c.get("proxy").cloned())
            .filter(|p| !p.is_empty())
    };

    let result = tokio::task::spawn_blocking(move || {
        let (client, _) = create_client(proxy.as_deref()).map_err(|e| e.to_string())?;
        let resp = client
            .get("https://ipinfo.io/json")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .map_err(|e| format!("请求失败: {}", e))?;
        let status = resp.status().as_u16();
        let text = resp.text().unwrap_or_default();
        if status >= 400 {
            return Err(format!("HTTP {}: {}", status, text));
        }
        let data: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("解析响应失败: {}", e))?;
        Ok(ProxyTestResult {
            ip: data.get("ip").and_then(|v| v.as_str()).unwrap_or("-").to_string(),
            country: data.get("country").and_then(|v| v.as_str()).unwrap_or("-").to_string(),
            city: data.get("city").and_then(|v| v.as_str()).unwrap_or("-").to_string(),
        })
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))??;

    Ok(result)
}

#[tauri::command]
pub async fn test_mail_gateway_health(
    base_url: String,
    api_key: Option<String>,
) -> Result<MailGatewayHealthResult, String> {
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    if base_url.is_empty() {
        return Err("Base URL 不能为空".to_string());
    }
    let api_key = api_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let url = format!("{}/health", base_url);

    let result = tokio::task::spawn_blocking(move || {
        let (client, _) = create_client(None).map_err(|e| e.to_string())?;
        let mut request = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(15));
        if let Some(token) = api_key.as_deref() {
            request = request.header("X-API-Key", token);
        }
        let resp = request
            .send()
            .map_err(|e| format!("请求失败: {}", e))?;
        let status_code = resp.status().as_u16();
        let text = resp.text().unwrap_or_default();
        if status_code >= 400 {
            return Err(format!("HTTP {}: {}", status_code, text));
        }

        let data: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("解析响应失败: {}", e))?;
        let status = data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let timestamp = data
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let providers = data
            .get("providers")
            .and_then(|value| value.as_object())
            .ok_or_else(|| "健康检查失败: 响应缺少 providers".to_string())?
            .iter()
            .map(|(provider, status_value)| {
                status_value
                    .as_str()
                    .map(|status| (provider.clone(), status.to_string()))
                    .ok_or_else(|| format!("健康检查失败: providers.{} 不是字符串", provider))
            })
            .collect::<Result<HashMap<String, String>, String>>()?;

        if status != "ok" {
            return Err(format!("健康检查失败: status={}", status));
        }
        if timestamp <= 0 {
            return Err("健康检查失败: 响应缺少有效 timestamp".to_string());
        }

        Ok(MailGatewayHealthResult {
            status,
            timestamp,
            providers,
        })
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))??;

    Ok(result)
}
