package com.tingyuxuan.ime

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
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

    // 振幅更新 Job
    private var amplitudeJob: Job? = null

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
        // 切换输入框时停止录音
        if (recordingController.isRecording) {
            recordingController.cancel()
        }
        amplitudeJob?.cancel()
        val current = _state.value
        if (current is IMEState.Recording || current is IMEState.Processing) {
            _state.value = IMEState.Idle(currentMode = currentMode)
        }
    }

    override fun onWindowHidden() {
        super.onWindowHidden()
        // 键盘隐藏时取消录音
        if (recordingController.isRecording) {
            recordingController.cancel()
        }
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

        val error = recordingController.start(mode)
        if (error != null) {
            _state.value = IMEState.Error(error, errorMessage(error), failedMode = mode)
            return
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

    private fun stopRecording() {
        amplitudeJob?.cancel()
        val mode = currentMode
        val audioPath = recordingController.stop() ?: run {
            _state.value = IMEState.Idle(currentMode = mode)
            return
        }

        _state.value = IMEState.Processing(mode = mode, stage = ProcessingStage.Transcribing)

        scope.launch {
            val result = pipelineController.processAudio(audioPath, mode)
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

        // 1.5 秒后回到 Idle
        scope.launch(Dispatchers.Main) {
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
