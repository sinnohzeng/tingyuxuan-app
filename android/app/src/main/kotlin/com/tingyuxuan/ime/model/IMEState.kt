package com.tingyuxuan.ime.model

/**
 * IME 状态机 — 用 sealed class 建模所有可能状态。
 *
 * 设计原则：
 * - 永远不用 Boolean flag 组合管理状态
 * - 每个状态携带该状态所需的全部数据
 * - UI = f(IMEState)，纯函数式渲染
 */
sealed class IMEState {
    /** 空闲状态，等待用户操作 */
    data class Idle(
        val isPasswordField: Boolean = false,
        val currentMode: ProcessingMode = ProcessingMode.Dictate,
    ) : IMEState()

    /** 正在录音 */
    data class Recording(
        val mode: ProcessingMode,
        val startTimeMs: Long = System.currentTimeMillis(),
        val amplitude: Float = 0f,
    ) : IMEState()

    /** 录音完成，正在处理（单步多模态） */
    data class Processing(
        val mode: ProcessingMode,
        val stage: ProcessingStage = ProcessingStage.Thinking,
    ) : IMEState()

    /** 处理完成，文本已输出 */
    data class Done(
        val text: String,
        val mode: ProcessingMode,
    ) : IMEState()

    /** 出错 */
    data class Error(
        val code: ErrorCode,
        val message: String,
        val failedMode: ProcessingMode = ProcessingMode.Dictate,
    ) : IMEState()
}

/**
 * 处理模式 — 对应 Rust 核心的 ProcessingMode。
 */
enum class ProcessingMode(val id: String, val label: String) {
    Dictate("dictate", "听写"),
    Translate("translate", "翻译"),
    AiAssistant("ai_assistant", "AI"),
    Edit("edit", "编辑");
}

/**
 * 处理阶段 — 用于 Processing 状态的进度展示。
 */
enum class ProcessingStage(val label: String) {
    Thinking("正在思考..."),
    Finalizing("正在整理结果..."),
}

/**
 * 错误码 — 类型化的错误分类，用于 UI 展示和用户操作指引。
 */
enum class ErrorCode(val userAction: UserAction) {
    PermissionDenied(UserAction.RequestPermission),
    ApiKeyMissing(UserAction.OpenSettings),
    NativeLibraryMissing(UserAction.Reinstall),
    NetworkError(UserAction.Retry),
    ProviderAuthFailed(UserAction.CheckApiKey),
    Timeout(UserAction.Retry),
    Unknown(UserAction.Dismiss),
}

/**
 * 用户操作建议 — 错误发生后指引用户下一步。
 */
enum class UserAction(val buttonLabel: String) {
    RequestPermission("授权"),
    OpenSettings("设置"),
    Reinstall("重新安装"),
    Retry("重试"),
    CheckApiKey("检查 API Key"),
    Dismiss("关闭"),
}
