package com.tingyuxuan.ime.controller

import android.content.Context
import android.util.Log
import com.tingyuxuan.core.NativeCore
import com.tingyuxuan.ime.ConfigStore
import com.tingyuxuan.ime.model.ErrorCode
import com.tingyuxuan.ime.model.ProcessingMode
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject

/**
 * Pipeline 控制器 — 封装 JNI 调用，管理 Pipeline 生命周期。
 *
 * 职责：
 * - 初始化/销毁 Pipeline handle
 * - 管理流式 STT 会话（startStreaming / sendAudioChunk / stopStreaming）
 * - 映射 Rust 错误到 Kotlin ErrorCode
 */
class PipelineController(private val context: Context) {

    companion object {
        private const val TAG = "PipelineController"
    }

    @Volatile
    private var pipelineHandle: Long = 0L

    /** Pipeline 是否已初始化 */
    val isInitialized: Boolean get() = pipelineHandle != 0L

    /** 是否有活跃的流式会话 */
    @Volatile
    var isStreaming: Boolean = false
        private set

    /** ConfigStore 实例缓存，避免每次 initialize() 重建 EncryptedSharedPreferences */
    private val configStore: ConfigStore by lazy { ConfigStore(context) }

    /**
     * 初始化 Pipeline。
     *
     * @return null 表示成功，非 null 表示错误码
     */
    fun initialize(): ErrorCode? {
        if (!NativeCore.isLoaded) {
            Log.e(TAG, "Native library not loaded")
            return ErrorCode.NativeLibraryMissing
        }

        val configJson = configStore.buildConfigJson()
        if (configJson == null) {
            Log.w(TAG, "API keys not configured")
            return ErrorCode.ApiKeyMissing
        }

        pipelineHandle = NativeCore.initPipeline(configJson)
        if (pipelineHandle == 0L) {
            Log.e(TAG, "Failed to initialize pipeline")
            return ErrorCode.ApiKeyMissing
        }

        Log.i(TAG, "Pipeline initialized with handle=$pipelineHandle")
        return null
    }

    /**
     * 重新初始化 Pipeline（配置变更后调用）。
     */
    fun reinitialize(): ErrorCode? {
        destroy()
        return initialize()
    }

    /**
     * 开始流式 STT 会话。
     *
     * 建立 WebSocket 连接，准备接收音频帧。
     * 调用后通过 [sendAudioChunk] 发送 PCM 数据。
     *
     * @param mode 处理模式
     * @param contextJson InputContext 的 JSON 序列化（空字符串表示无上下文）
     * @return null 表示成功，非 null 表示失败
     */
    fun startStreaming(
        mode: ProcessingMode,
        contextJson: String = "",
    ): ProcessResult.Failure? {
        if (!isInitialized) {
            return ProcessResult.Failure(
                ErrorCode.ApiKeyMissing,
                "Pipeline 未初始化，请先配置 API Key",
            )
        }

        return try {
            val resultJson = NativeCore.startStreaming(
                pipelineHandle,
                mode.id,
                contextJson,
            )
            val obj = JSONObject(resultJson)
            if (obj.optBoolean("success", false)) {
                isStreaming = true
                Log.i(TAG, "Streaming started")
                null
            } else {
                val errorCode = obj.optString("error_code", "unknown")
                val message = obj.optString("message", "流式连接失败")
                ProcessResult.Failure(mapErrorCode(errorCode), message)
            }
        } catch (e: Exception) {
            Log.e(TAG, "startStreaming exception", e)
            ProcessResult.Failure(ErrorCode.Unknown, e.message ?: "未知错误")
        }
    }

    /**
     * 发送一帧 PCM 音频数据到流式 STT。
     *
     * @param pcmData 16kHz mono PCM16 音频数据
     * @return true 表示发送成功
     */
    fun sendAudioChunk(pcmData: ShortArray): Boolean {
        val handle = pipelineHandle
        if (handle == 0L || !isStreaming) return false
        return try {
            NativeCore.sendAudioChunk(handle, pcmData)
        } catch (e: Exception) {
            Log.e(TAG, "sendAudioChunk exception", e)
            false
        }
    }

    /**
     * 停止流式录音，收集 STT 结果并执行 LLM 处理。
     *
     * 阻塞调用 — 在 IO 线程中执行。
     *
     * @return [ProcessResult.Success] 或 [ProcessResult.Failure]
     */
    suspend fun stopStreaming(): ProcessResult = withContext(Dispatchers.IO) {
        isStreaming = false

        if (!isInitialized) {
            return@withContext ProcessResult.Failure(
                ErrorCode.ApiKeyMissing,
                "Pipeline 未初始化",
            )
        }

        try {
            val resultJson = NativeCore.stopStreaming(pipelineHandle)
            parseResult(resultJson)
        } catch (e: Exception) {
            Log.e(TAG, "stopStreaming exception", e)
            ProcessResult.Failure(ErrorCode.Unknown, e.message ?: "未知错误")
        }
    }

    /**
     * 取消当前流式会话。
     */
    fun cancelStreaming() {
        if (!isStreaming) return
        isStreaming = false
        val handle = pipelineHandle
        if (handle == 0L) return
        try {
            NativeCore.cancelProcessing(handle)
            Log.i(TAG, "Streaming cancelled")
        } catch (e: Exception) {
            Log.e(TAG, "cancelStreaming exception", e)
        }
    }

    /**
     * 销毁 Pipeline，释放资源。
     */
    fun destroy() {
        isStreaming = false
        if (pipelineHandle != 0L) {
            NativeCore.destroyPipeline(pipelineHandle)
            Log.i(TAG, "Pipeline destroyed (handle=$pipelineHandle)")
            pipelineHandle = 0L
        }
    }

    private fun parseResult(json: String): ProcessResult {
        return try {
            val obj = JSONObject(json)
            if (obj.optBoolean("success", false)) {
                ProcessResult.Success(obj.getString("text"))
            } else {
                val errorCode = obj.optString("error_code", "unknown")
                val message = obj.optString("message", obj.optString("error", "处理失败"))
                ProcessResult.Failure(mapErrorCode(errorCode), message)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse result JSON: $json", e)
            ProcessResult.Failure(ErrorCode.Unknown, "结果解析失败")
        }
    }

    private fun mapErrorCode(code: String): ErrorCode = when (code) {
        "stt_auth_failed" -> ErrorCode.SttAuthFailed
        "llm_auth_failed" -> ErrorCode.LlmAuthFailed
        "timeout" -> ErrorCode.Timeout
        "network_error" -> ErrorCode.NetworkError
        "not_configured" -> ErrorCode.ApiKeyMissing
        "cancelled" -> ErrorCode.Unknown
        "busy" -> ErrorCode.Unknown
        "audio_error" -> ErrorCode.Unknown
        else -> ErrorCode.Unknown
    }
}

/** 音频处理结果 */
sealed class ProcessResult {
    data class Success(val text: String) : ProcessResult()
    data class Failure(val errorCode: ErrorCode, val message: String) : ProcessResult()
}
