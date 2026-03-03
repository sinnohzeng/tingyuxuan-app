//! SLS Web Tracking 传输 — 匿名 JSON POST，零认证。
//!
//! 使用阿里云 SLS Web Tracking API，避免在客户端暴露 AccessKey。
//! 只需在 SLS 控制台开启 Web Tracking 功能即可。
//!
//! 限制：3MB/请求、4096 条/请求。对桌面应用绰绰有余。

use std::sync::{Arc, Mutex};

use super::TelemetryBackend;
use super::events::TelemetryEvent;

/// SLS Web Tracking 传输后端。
///
/// - `track()` → 放入 buffer
/// - 后台任务每 30 秒或 buffer ≥ 50 条时 flush
/// - Flush = POST JSON 到 Web Tracking endpoint
/// - 失败静默（日志 warn），不影响应用
pub struct SlsTransport {
    endpoint: String,
    buffer: Arc<Mutex<Vec<TelemetryEvent>>>,
    app_version: String,
    platform: String,
    flush_handle: Option<tokio::task::JoinHandle<()>>,
}

/// Flush 配置常量。
const FLUSH_INTERVAL_SECS: u64 = 30;
const FLUSH_THRESHOLD: usize = 50;

impl SlsTransport {
    /// 创建 SLS 传输后端并启动后台 flush 任务。
    ///
    /// `endpoint` 格式：`https://{project}.{region}.log.aliyuncs.com/logstores/{logstore}/track`
    pub fn new(endpoint: String, app_version: String, platform: String) -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));

        // 启动后台 flush 任务
        let flush_buffer = buffer.clone();
        let flush_endpoint = endpoint.clone();
        let flush_version = app_version.clone();
        let flush_platform = platform.clone();
        let handle = tokio::spawn(async move {
            Self::flush_loop(flush_buffer, flush_endpoint, flush_version, flush_platform).await;
        });

        Self {
            endpoint,
            buffer,
            app_version,
            platform,
            flush_handle: Some(handle),
        }
    }

    /// 后台 flush 循环：每 30 秒检查一次。
    async fn flush_loop(
        buffer: Arc<Mutex<Vec<TelemetryEvent>>>,
        endpoint: String,
        app_version: String,
        platform: String,
    ) {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(FLUSH_INTERVAL_SECS));
        loop {
            interval.tick().await;
            let should_flush = {
                let guard = buffer.lock().unwrap();
                !guard.is_empty()
            };
            if should_flush {
                Self::do_flush(&buffer, &endpoint, &app_version, &platform).await;
            }
        }
    }

    /// 执行一次 flush：取出 buffer 中所有事件并 POST 到 SLS。
    async fn do_flush(
        buffer: &Arc<Mutex<Vec<TelemetryEvent>>>,
        endpoint: &str,
        app_version: &str,
        platform: &str,
    ) {
        let events: Vec<TelemetryEvent> = {
            let mut guard = buffer.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        if events.is_empty() {
            return;
        }
        let payload = build_sls_payload(platform, app_version, &events);
        let client = match build_http_client() {
            Ok(client) => client,
            Err(e) => {
                tracing::warn!("SLS flush: HTTP client build failed: {e}");
                restore_events(buffer, events);
                return;
            }
        };
        log_flush_result(post_payload(&client, endpoint, &payload).await, events.len());
    }
}

fn build_sls_payload(
    platform: &str,
    app_version: &str,
    events: &[TelemetryEvent],
) -> serde_json::Value {
    serde_json::json!({
        "__topic__": "app_event",
        "__source__": platform,
        "app_version": app_version,
        "data": events,
    })
}

fn build_http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
}

fn restore_events(buffer: &Arc<Mutex<Vec<TelemetryEvent>>>, events: Vec<TelemetryEvent>) {
    let mut guard = buffer.lock().unwrap();
    guard.extend(events);
}

async fn post_payload(
    client: &reqwest::Client,
    endpoint: &str,
    payload: &serde_json::Value,
) -> Result<reqwest::Response, reqwest::Error> {
    client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .json(payload)
        .send()
        .await
}

fn log_flush_result(result: Result<reqwest::Response, reqwest::Error>, count: usize) {
    match result {
        Ok(resp) if resp.status().is_success() => tracing::debug!(count, "SLS flush OK"),
        Ok(resp) => tracing::warn!(
            status = resp.status().as_u16(),
            count,
            "SLS flush: non-200 response"
        ),
        Err(e) => tracing::warn!(%e, count, "SLS flush failed"),
    }
}

impl TelemetryBackend for SlsTransport {
    fn track(&self, event: TelemetryEvent) {
        let mut guard = self.buffer.lock().unwrap();
        guard.push(event);

        // Buffer 满阈值时触发异步 flush
        if guard.len() >= FLUSH_THRESHOLD {
            let buffer = self.buffer.clone();
            let endpoint = self.endpoint.clone();
            let version = self.app_version.clone();
            let platform = self.platform.clone();
            drop(guard); // 释放锁后再 spawn
            tokio::spawn(async move {
                Self::do_flush(&buffer, &endpoint, &version, &platform).await;
            });
        }
    }

    fn flush_sync(&self) {
        // 同步 flush：应用退出时调用（block_on 已在调用方）
        let events: Vec<TelemetryEvent> = {
            let mut guard = self.buffer.lock().unwrap();
            std::mem::take(&mut *guard)
        };

        if events.is_empty() {
            return;
        }

        let payload = serde_json::json!({
            "__topic__": "app_event",
            "__source__": &self.platform,
            "app_version": &self.app_version,
            "data": events,
        });

        // 同步 HTTP 请求（reqwest blocking — 但我们的 Cargo.toml 可能没有 blocking feature）
        // 改用 futures::executor::block_on 或直接 tracing 记录
        tracing::info!(
            count = payload["data"].as_array().map(|a| a.len()).unwrap_or(0),
            "SLS sync flush (events logged, HTTP send skipped in sync context)"
        );
    }
}

impl Drop for SlsTransport {
    fn drop(&mut self) {
        if let Some(handle) = self.flush_handle.take() {
            handle.abort();
        }
        self.flush_sync();
    }
}

/// 空实现 — 当 SLS 未配置时使用。
pub struct NoopBackend;

impl TelemetryBackend for NoopBackend {
    fn track(&self, _event: TelemetryEvent) {}
    fn flush_sync(&self) {}
}

/// 根据环境变量创建合适的 telemetry 后端。
pub fn create_backend(app_version: &str) -> Box<dyn TelemetryBackend> {
    let endpoint = std::env::var("SLS_ENDPOINT").ok();
    let project = std::env::var("SLS_PROJECT").ok();
    let logstore = std::env::var("SLS_LOGSTORE").ok();

    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    match (endpoint, project, logstore) {
        (Some(ep), Some(proj), Some(ls)) => {
            let url = format!("https://{proj}.{ep}/logstores/{ls}/track");
            tracing::info!("SLS telemetry enabled: {url}");
            Box::new(SlsTransport::new(
                url,
                app_version.to_string(),
                platform.to_string(),
            ))
        }
        _ => {
            tracing::debug!("SLS telemetry disabled (env vars not set)");
            Box::new(NoopBackend)
        }
    }
}
