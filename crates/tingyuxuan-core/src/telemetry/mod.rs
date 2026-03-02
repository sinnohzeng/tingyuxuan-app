//! Telemetry 模块 — trait 抽象 + 可替换后端。
//!
//! 事件定义与传输解耦。应用层调用 `TelemetryBackend::track()` 上报事件，
//! 具体传输由后端实现决定（SLS Web Tracking、本地日志等）。

pub mod events;
pub mod sls;

pub use events::TelemetryEvent;

/// Telemetry 后端 trait — 事件上报接口。
///
/// 所有方法都是非阻塞的：`track()` 放入缓冲区，后台异步 flush。
pub trait TelemetryBackend: Send + Sync {
    /// 上报一个事件（非阻塞，放入内部缓冲区）。
    fn track(&self, event: TelemetryEvent);

    /// 立即 flush 缓冲区中的事件（应用退出时调用）。
    fn flush_sync(&self);
}
