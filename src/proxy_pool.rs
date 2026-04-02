use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;

use crate::constants::user_agent;
use crate::errors::AppError;
use crate::http_client::req_timeout_secs;

/// 代理池结构
#[derive(Clone)]
pub struct ProxyPool {
    proxies: Arc<Mutex<Vec<ProxyEntry>>>,
    api_url: String,
    fetch_lock: Arc<Mutex<i64>>, // 存储上次获取的时间戳，用于防止并发获取和限速
}

#[derive(Clone, Debug)]
struct ProxyEntry {
    proxy: String,
    expires_at: i64,
}

impl ProxyPool {
    /// 创建新的代理池
    pub fn new(api_url: String) -> Self {
        Self {
            proxies: Arc::new(Mutex::new(Vec::new())),
            api_url,
            fetch_lock: Arc::new(Mutex::new(0)),
        }
    }

    /// 从 API 获取代理列表
    fn fetch_proxies(&self, client: &Client, timeout: i64) -> Result<Vec<String>, AppError> {
        let resp = client
            .get(&self.api_url)
            .header("user-agent", user_agent())
            .timeout(req_timeout_secs(timeout))
            .send()
            .map_err(|e| AppError::Runtime(format!("获取代理失败: {}", e)))?;

        let status = resp.status().as_u16();
        if status >= 400 {
            return Err(AppError::Runtime(format!("获取代理失败: HTTP {}", status)));
        }

        let text = resp.text().map_err(|e| AppError::Runtime(format!("读取代理响应失败: {}", e)))?;

        // 解析格式: ip|port 或 ip:port，每行一个
        let proxies: Vec<String> = text
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }

                // 支持 ip|port 和 ip:port 两种格式
                let proxy = if line.contains('|') {
                    line.replace('|', ":")
                } else {
                    line.to_string()
                };

                // 添加 http:// 前缀
                if proxy.starts_with("http://") || proxy.starts_with("socks5://") {
                    Some(proxy)
                } else {
                    Some(format!("http://{}", proxy))
                }
            })
            .collect();

        if proxies.is_empty() {
            return Err(AppError::Runtime("代理 API 返回空列表".to_string()));
        }

        Ok(proxies)
    }

    /// 获取一个可用的代理
    pub fn get_proxy(&self, client: &Client, timeout: i64) -> Result<String, AppError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // 先尝试从现有池中获取
        {
            let mut proxies = self.proxies.lock().unwrap();
            proxies.retain(|p| p.expires_at > now);

            if let Some(entry) = proxies.first() {
                let proxy = entry.proxy.clone();
                proxies.remove(0);
                return Ok(proxy);
            }
        }

        // 池为空，需要获取新代理
        // 使用 fetch_lock 确保同一时间只有一个线程在获取
        let mut last_fetch_time = self.fetch_lock.lock().unwrap();

        // 再次检查池（可能其他线程已经填充了）
        {
            let mut proxies = self.proxies.lock().unwrap();
            proxies.retain(|p| p.expires_at > now);

            if let Some(entry) = proxies.first() {
                let proxy = entry.proxy.clone();
                proxies.remove(0);
                return Ok(proxy);
            }
        }

        // 确保距离上次获取至少 1.5 秒（留出余量）
        let elapsed = now - *last_fetch_time;
        if elapsed < 2 {
            let sleep_duration = (2 - elapsed) as u64;
            std::thread::sleep(Duration::from_secs(sleep_duration));
        }

        // 获取新代理
        let new_proxies = self.fetch_proxies(client, timeout)?;
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64 + 60;

        *last_fetch_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let mut proxies = self.proxies.lock().unwrap();
        proxies.clear();
        for proxy in new_proxies {
            proxies.push(ProxyEntry {
                proxy,
                expires_at,
            });
        }

        // 取出第一个代理
        if let Some(entry) = proxies.first() {
            let proxy = entry.proxy.clone();
            proxies.remove(0);
            Ok(proxy)
        } else {
            Err(AppError::Runtime("无可用代理".to_string()))
        }
    }

    /// 获取剩余代理数量
    pub fn remaining_count(&self) -> usize {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let proxies = self.proxies.lock().unwrap();
        proxies.iter().filter(|p| p.expires_at > now).count()
    }
}
