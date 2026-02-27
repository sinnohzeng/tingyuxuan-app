package com.tingyuxuan.ime.model

/**
 * 键盘 UI 发出的意图（MVI Intent）。
 *
 * UI 层只能通过发送 Intent 来请求状态变更，
 * 不直接操作业务逻辑。
 */
sealed class KeyboardIntent {
    /** 开始录音 */
    data class StartRecording(val mode: ProcessingMode) : KeyboardIntent()

    /** 停止录音并开始处理 */
    data object StopRecording : KeyboardIntent()

    /** 取消当前录音（不处理） */
    data object CancelRecording : KeyboardIntent()

    /** 切换处理模式 */
    data class SwitchMode(val mode: ProcessingMode) : KeyboardIntent()

    /** 打开设置页面 */
    data object OpenSettings : KeyboardIntent()

    /** 清除错误状态，回到 Idle */
    data object ClearError : KeyboardIntent()

    /** 重试上次失败的操作 */
    data object Retry : KeyboardIntent()
}
