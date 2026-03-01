//! Session 结果类型 — 描述一次录音 session 的最终结果。

use crate::error::PipelineError;

/// Session 处理结果。
#[derive(Debug)]
pub enum SessionResult {
    /// 处理成功。
    Success { processed_text: String },
    /// 音频为空（录音时长过短）。
    EmptyAudio,
    /// 处理失败。
    Failed { error: PipelineError },
    /// 用户取消。
    Cancelled,
}
