package com.tingyuxuan.ime

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.text.InputType
import android.util.Log
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.Toast
import com.tingyuxuan.ime.controller.PipelineController
import com.tingyuxuan.ime.controller.ProcessResult
import com.tingyuxuan.ime.controller.RecordingController
import com.tingyuxuan.ime.model.*
import com.tingyuxuan.ime.ui.TingYuXuanKeyboard
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * 听语轩输入法服务 — 基于 MVI 架构的完整 IME 实现。
 *
 * 架构：
 * - State: [IMEState] sealed class 状态机
 * - Intent: [KeyboardIntent] 从 UI 发出
 * - Reducer: [handleIntent] 处理意图并更新状态
 *
 * 生命周期：
 * - onCreate: 初始化 Pipeline
 * - onCreateInputView: 创建 Compose 键盘 UI
 * - onStartInput: 检测输入框类型（密码字段禁用语音）
 * - onFinishInput: 停止录音，清理状态
 * - onWindowHidden: 暂停录音
 * - onDestroy: 释放所有资源
 */
class TingYuXuanIMEService : LifecycleInputMethodService() {

    companion object {
        private const val TAG = "TingYuXuanIME"
        private const val DONE_DISPLAY_MS = 1500L
    }

    private lateinit var pipelineController: PipelineController
    private lateinit var recordingController: RecordingController

    private val _state = MutableStateFlow<IMEState>(IMEState.Idle())
    val state: StateFlow<IMEState> = _state.asStateFlow()

    /**
     * 从当前状态中提取处理模式。
     * 每个状态自带模式信息，无需额外字段。
     */
    private val currentMode: ProcessingMode
        get() = when (val s = _state.value) {
            is IMEState.Idle -> s.currentMode
            is IMEState.Recording -> s.mode
            is IMEState.Processing -> s.mode
            is IMEState.Done -> s.mode
            is IMEState.Error -> s.failedMode
        }

    // 当前输入框信息（onStartInput 时更新）
    private var currentEditorInfo: EditorInfo? = null

    // 振幅更新 Job
    private var amplitudeJob: Job? = null

    // Done 状态延迟回退 Job（防止多次触发累积僵尸协程）
    private var doneTimerJob: Job? = null

    private val exceptionHandler = CoroutineExceptionHandler { _, throwable ->
        Log.e(TAG, "Uncaught coroutine exception", throwable)
        _state.value = IMEState.Error(ErrorCode.Unknown, throwable.message ?: "未知错误", failedMode = currentMode)
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO + exceptionHandler)

    override fun onCreate() {
        super.onCreate()
        pipelineController = PipelineController(this)
        recordingController = RecordingController(this)

        // 初始化 Pipeline
        val error = pipelineController.initialize()
        if (error != null) {
            Log.w(TAG, "Pipeline init deferred: $error")
            // 不立即设置 Error 状态 — 用户可能稍后配置
        }
    }

    override fun onCreateInputView(): View {
        val view = super.onCreateInputView() ?: TingYuXuanKeyboard.createView(
            context = this,
            state = state,
            lifecycleOwner = this,
            onIntent = ::handleIntent,
        )
        view.installViewTreeOwners()
        return view
    }

    override fun onStartInput(attribute: EditorInfo?, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        currentEditorInfo = attribute

        // 检测密码字段 — 在密码框中禁用语音输入
        val isPassword = attribute?.let {
            val inputClass = it.inputType and InputType.TYPE_MASK_CLASS
            val variation = it.inputType and InputType.TYPE_MASK_VARIATION
            inputClass == InputType.TYPE_CLASS_TEXT && (
                variation == InputType.TYPE_TEXT_VARIATION_PASSWORD ||
                variation == InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD ||
                variation == InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
            )
        } ?: false

        // 仅当不在录音/处理中时重置状态
        val current = _state.value
        if (current is IMEState.Idle || current is IMEState.Done || current is IMEState.Error) {
            _state.value = IMEState.Idle(isPasswordField = isPassword, currentMode = currentMode)
        }
    }

    override fun onFinishInput() {
        super.onFinishInput()
        // 切换输入框时停止录音并取消流式会话
        if (recordingController.isRecording) {
            recordingController.cancel()
        }
        pipelineController.cancelStreaming()
        amplitudeJob?.cancel()
        val current = _state.value
        if (current is IMEState.Recording || current is IMEState.Processing) {
            _state.value = IMEState.Idle(currentMode = currentMode)
        }
    }

    override fun onWindowHidden() {
        super.onWindowHidden()
        // 键盘隐藏时取消录音并取消流式会话
        if (recordingController.isRecording) {
            recordingController.cancel()
        }
        pipelineController.cancelStreaming()
        amplitudeJob?.cancel()
        val current = _state.value
        if (current is IMEState.Recording) {
            _state.value = IMEState.Idle(currentMode = currentMode)
        }
    }

    override fun onDestroy() {
        amplitudeJob?.cancel()
        scope.cancel()
        recordingController.cancel()
        pipelineController.destroy()
        super.onDestroy()
    }

    /**
     * MVI Reducer — 处理键盘 UI 发出的 Intent。
     */
    fun handleIntent(intent: KeyboardIntent) {
        when (intent) {
            is KeyboardIntent.StartRecording -> startRecording(intent.mode)
            is KeyboardIntent.StopRecording -> stopRecording()
            is KeyboardIntent.CancelRecording -> cancelRecording()
            is KeyboardIntent.SwitchMode -> switchMode(intent.mode)
            is KeyboardIntent.OpenSettings -> openSettings()
            is KeyboardIntent.ClearError -> clearError()
            is KeyboardIntent.Retry -> retry()
        }
    }

    private fun startRecording(mode: ProcessingMode) {
        val currentState = _state.value

        // 密码字段禁止录音
        if (currentState is IMEState.Idle && currentState.isPasswordField) {
            _state.value = IMEState.Error(ErrorCode.Unknown, "密码输入框中不可使用语音输入", failedMode = mode)
            return
        }

        // Pipeline 未初始化时尝试重新初始化
        if (!pipelineController.isInitialized) {
            val error = pipelineController.reinitialize()
            if (error != null) {
                _state.value = IMEState.Error(error, errorMessage(error), failedMode = mode)
                return
            }
        }

        // 采集上下文（在主线程，因为 InputConnection 只能在 IME 线程访问）
        val contextJson = collectContext().toJson()

        // 1. 建立流式 STT 连接（在 IO 线程，避免阻塞主线程）
        scope.launch {
            val streamError = pipelineController.startStreaming(mode, contextJson)
            if (streamError != null) {
                withContext(Dispatchers.Main) {
                    _state.value = IMEState.Error(streamError.errorCode, streamError.message, failedMode = mode)
                }
                return@launch
            }

            withContext(Dispatchers.Main) {
                // 状态守卫：IO→Main 切换期间可能有取消操作
                if (_state.value !is IMEState.Idle) {
                    pipelineController.cancelStreaming()
                    return@withContext
                }

                // 2. 开始录音，PCM 帧实时转发到 STT
                val recordError = recordingController.start(mode) { pcmData ->
                    pipelineController.sendAudioChunk(pcmData)
                }
                if (recordError != null) {
                    pipelineController.cancelStreaming()
                    _state.value = IMEState.Error(recordError, errorMessage(recordError), failedMode = mode)
                    return@withContext
                }

                _state.value = IMEState.Recording(mode = mode)

                // 启动振幅更新（~20fps）
                amplitudeJob?.cancel()
                amplitudeJob = scope.launch(Dispatchers.Main) {
                    while (isActive && recordingController.isRecording) {
                        val current = _state.value
                        if (current is IMEState.Recording) {
                            _state.value = current.copy(amplitude = recordingController.amplitude)
                        }
                        delay(50)
                    }
                }
            }
        }
    }

    private fun stopRecording() {
        amplitudeJob?.cancel()
        val mode = currentMode

        // 1. 停止录音 — 音频帧停止发送，STT 收到结束信号
        recordingController.stop()

        _state.value = IMEState.Processing(mode = mode, stage = ProcessingStage.Transcribing)

        // 2. 收集 STT 结果 → LLM 处理（阻塞调用，在 IO 线程）
        scope.launch {
            val result = pipelineController.stopStreaming()
            withContext(Dispatchers.Main) {
                when (result) {
                    is ProcessResult.Success -> onProcessingSuccess(result.text, mode)
                    is ProcessResult.Failure -> onProcessingFailure(result.errorCode, result.message, mode)
                }
            }
        }
    }

    private fun cancelRecording() {
        amplitudeJob?.cancel()
        val mode = currentMode
        recordingController.cancel()
        pipelineController.cancelStreaming()
        _state.value = IMEState.Idle(currentMode = mode)
    }

    private fun switchMode(mode: ProcessingMode) {
        val current = _state.value
        if (current is IMEState.Idle) {
            _state.value = current.copy(currentMode = mode)
        }
    }

    private fun openSettings() {
        val intent = Intent(this, SettingsActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        startActivity(intent)
    }

    private fun clearError() {
        _state.value = IMEState.Idle(currentMode = currentMode)
    }

    private fun retry() {
        val current = _state.value
        val mode = if (current is IMEState.Error) current.failedMode else currentMode
        startRecording(mode)
    }

    private fun onProcessingSuccess(text: String, mode: ProcessingMode) {
        val connection = currentInputConnection
        if (connection != null) {
            connection.commitText(text, 1)
        } else {
            // InputConnection 不可用 — 复制到剪贴板
            copyToClipboard(text)
            showToast("结果已复制到剪贴板")
        }

        _state.value = IMEState.Done(text = text, mode = mode)

        // 1.5 秒后回到 Idle（取消旧任务防止僵尸协程累积）
        doneTimerJob?.cancel()
        doneTimerJob = scope.launch(Dispatchers.Main) {
            delay(DONE_DISPLAY_MS)
            if (_state.value is IMEState.Done) {
                _state.value = IMEState.Idle(currentMode = mode)
            }
        }
    }

    private fun onProcessingFailure(errorCode: ErrorCode, message: String, mode: ProcessingMode) {
        _state.value = IMEState.Error(errorCode, message, failedMode = mode)
    }

    private fun copyToClipboard(text: String) {
        val clipboard = getSystemService(CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newPlainText("听语轩", text))
    }

    private fun showToast(message: String) {
        Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
    }

    // ------------------------------------------------------------------
    // 上下文采集
    // ------------------------------------------------------------------

    /**
     * 采集当前输入上下文。
     *
     * 各项采集独立 try-catch，单项失败不影响其他字段。
     * 必须在 IME 主线程调用（InputConnection 线程安全限制）。
     */
    private fun collectContext(): InputContextData {
        val editorInfo = currentEditorInfo

        // 剪贴板 — 先于 selectedText 采集（避免后续操作覆盖）
        val clipboardText = try {
            val clipboard = getSystemService(CLIPBOARD_SERVICE) as? ClipboardManager
            clipboard?.primaryClip?.getItemAt(0)?.text?.toString()
        } catch (e: Exception) {
            Log.d(TAG, "Failed to read clipboard", e)
            null
        }

        // InputConnection 文本上下文
        val connection = currentInputConnection
        val surroundingText = try {
            if (connection != null) {
                val before = connection.getTextBeforeCursor(500, 0)?.toString() ?: ""
                val after = connection.getTextAfterCursor(500, 0)?.toString() ?: ""
                val combined = before + after
                combined.ifEmpty { null }
            } else null
        } catch (e: Exception) {
            Log.d(TAG, "Failed to read surrounding text", e)
            null
        }

        val selectedText = try {
            connection?.getSelectedText(0)?.toString()
        } catch (e: Exception) {
            Log.d(TAG, "Failed to read selected text", e)
            null
        }

        // 应用信息
        val appPackage = editorInfo?.packageName
        val appName = try {
            appPackage?.let { pkg ->
                val appInfo = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    packageManager.getApplicationInfo(pkg, PackageManager.ApplicationInfoFlags.of(0))
                } else {
                    @Suppress("DEPRECATION")
                    packageManager.getApplicationInfo(pkg, 0)
                }
                packageManager.getApplicationLabel(appInfo).toString()
            }
        } catch (e: Exception) {
            Log.d(TAG, "Failed to resolve app name for $appPackage", e)
            null
        }

        // 输入框类型
        val inputFieldType = editorInfo?.let { parseInputFieldType(it.inputType) }

        // hint 文本
        val inputHint = editorInfo?.hintText?.toString()

        // 编辑器动作
        val editorAction = editorInfo?.let { parseEditorAction(it.imeOptions) }

        return InputContextData(
            appName = appName,
            appPackage = appPackage,
            windowTitle = null, // Android 无窗口标题概念
            browserUrl = null, // 需要无障碍服务，本阶段不实现
            inputFieldType = inputFieldType,
            inputHint = inputHint,
            editorAction = editorAction,
            surroundingText = surroundingText,
            selectedText = selectedText,
            clipboardText = clipboardText,
            screenText = null, // 需要无障碍服务，本阶段不实现
        )
    }

    /**
     * 解析 Android inputType 位域为 InputFieldType 字符串（与 Rust 枚举一致）。
     */
    private fun parseInputFieldType(inputType: Int): String {
        val cls = inputType and InputType.TYPE_MASK_CLASS
        val variation = inputType and InputType.TYPE_MASK_VARIATION
        return when {
            cls == InputType.TYPE_CLASS_TEXT && variation == InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS -> "email"
            cls == InputType.TYPE_CLASS_TEXT && variation == InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS -> "email"
            cls == InputType.TYPE_CLASS_TEXT && variation == InputType.TYPE_TEXT_VARIATION_URI -> "url"
            cls == InputType.TYPE_CLASS_TEXT && variation == InputType.TYPE_TEXT_VARIATION_SHORT_MESSAGE -> "chat"
            cls == InputType.TYPE_CLASS_TEXT && variation == InputType.TYPE_TEXT_VARIATION_LONG_MESSAGE -> "chat"
            cls == InputType.TYPE_CLASS_TEXT && (inputType and InputType.TYPE_TEXT_FLAG_MULTI_LINE) != 0 -> "multiline"
            cls == InputType.TYPE_CLASS_TEXT && variation == InputType.TYPE_TEXT_VARIATION_FILTER -> "search"
            else -> "text"
        }
    }

    /**
     * 解析 imeOptions 为 EditorAction 字符串（与 Rust 枚举一致）。
     */
    private fun parseEditorAction(imeOptions: Int): String {
        return when (imeOptions and EditorInfo.IME_MASK_ACTION) {
            EditorInfo.IME_ACTION_SEND -> "send"
            EditorInfo.IME_ACTION_SEARCH -> "search"
            EditorInfo.IME_ACTION_GO -> "go"
            EditorInfo.IME_ACTION_DONE -> "done"
            EditorInfo.IME_ACTION_NEXT -> "next"
            else -> "unspecified"
        }
    }

    private fun errorMessage(code: ErrorCode): String = when (code) {
        ErrorCode.PermissionDenied -> "请授予录音权限"
        ErrorCode.ApiKeyMissing -> "请先配置 API Key"
        ErrorCode.NativeLibraryMissing -> "核心库加载失败，请重新安装"
        ErrorCode.NetworkError -> "网络连接失败，请检查网络"
        ErrorCode.SttAuthFailed -> "语音识别 API Key 无效"
        ErrorCode.LlmAuthFailed -> "语言模型 API Key 无效"
        ErrorCode.Timeout -> "请求超时，请重试"
        ErrorCode.Unknown -> "发生未知错误"
    }
}
