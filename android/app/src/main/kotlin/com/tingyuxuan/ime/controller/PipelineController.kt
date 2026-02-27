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
 * - 调用 processAudio 并解析返回值
 * - 映射 Rust 错误到 Kotlin ErrorCode
 */
class PipelineController(private val context: Context) {

    companion object {
        private const val TAG = "PipelineController"
    }

    private var pipelineHandle: Long = 0L

    /** Pipeline 是否已初始化 */
    val isInitialized: Boolean get() = pipelineHandle != 0L

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

        val configStore = ConfigStore(context)
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
     * 处理音频文件。
     *
     * @return [ProcessResult.Success] 或 [ProcessResult.Failure]
     */
    suspend fun processAudio(
        audioPath: String,
        mode: ProcessingMode,
        selectedText: String = "",
    ): ProcessResult = withContext(Dispatchers.IO) {
        if (!isInitialized) {
            return@withContext ProcessResult.Failure(
                ErrorCode.ApiKeyMissing,
                "Pipeline 未初始化，请先配置 API Key",
            )
        }

        try {
            val resultJson = NativeCore.processAudio(
                pipelineHandle,
                audioPath,
                mode.id,
                selectedText,
            )
            parseResult(resultJson)
        } catch (e: Exception) {
            Log.e(TAG, "processAudio exception", e)
            ProcessResult.Failure(ErrorCode.Unknown, e.message ?: "未知错误")
        }
    }

    /**
     * 销毁 Pipeline，释放资源。
     */
    fun destroy() {
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
                val userAction = obj.optString("user_action", "dismiss")
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
        else -> ErrorCode.Unknown
    }
}

/** 音频处理结果 */
sealed class ProcessResult {
    data class Success(val text: String) : ProcessResult()
    data class Failure(val errorCode: ErrorCode, val message: String) : ProcessResult()
}
