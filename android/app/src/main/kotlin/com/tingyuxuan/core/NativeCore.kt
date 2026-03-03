package com.tingyuxuan.core

import android.util.Log

/**
 * JNI interface to the Rust tingyuxuan-core engine.
 *
 * All methods are blocking — call from a coroutine dispatcher (Dispatchers.IO).
 *
 * 安全加载：如果 .so 文件缺失（构建问题），不会导致应用崩溃，
 * 而是将 [isLoaded] 设为 false，所有 JNI 方法调用前需检查此标志。
 */
object NativeCore {

    private const val TAG = "NativeCore"

    /** 原生库是否成功加载。调用任何 external fun 前必须检查。 */
    val isLoaded: Boolean

    init {
        isLoaded = try {
            System.loadLibrary("tingyuxuan_jni")
            Log.i(TAG, "Native library loaded successfully")
            true
        } catch (e: UnsatisfiedLinkError) {
            Log.e(TAG, "Failed to load native library: ${e.message}", e)
            false
        }
    }

    /**
     * 确保原生库已加载，否则抛出 [IllegalStateException]。
     */
    fun ensureLoaded() {
        check(isLoaded) { "Native library tingyuxuan_jni is not loaded" }
    }

    /**
     * Initialize a processing pipeline with the given configuration.
     *
     * @param configJson JSON string matching tingyuxuan-core's AppConfig schema.
     * @return Opaque pipeline handle (0 = failure).
     */
    external fun initPipeline(configJson: String): Long

    /**
     * 开始音频流会话，准备接收 PCM 音频帧。
     *
     * @param handle Pipeline handle from [initPipeline].
     * @param mode Processing mode: "dictate", "translate", "edit", "ai_assistant".
     * @param contextJson JSON string of InputContext (empty string if none).
     * @return JSON string: `{"success": true}` or error JSON.
     */
    external fun startStreaming(handle: Long, mode: String, contextJson: String): String

    /**
     * 发送一帧 PCM 音频到当前音频流会话。
     *
     * @param handle Pipeline handle.
     * @param pcmData 16kHz mono PCM16 音频数据。
     * @return true 表示发送成功，false 表示会话不存在或已关闭。
     */
    external fun sendAudioChunk(handle: Long, pcmData: ShortArray): Boolean

    /**
     * 停止录音并触发单步多模态处理。
     *
     * 阻塞调用 — 等待处理完成。
     *
     * @param handle Pipeline handle.
     * @return JSON string: `{"success": true, "text": "..."}` or error JSON.
     */
    external fun stopStreaming(handle: Long): String

    /**
     * Destroy a pipeline and free its resources.
     *
     * @param handle Pipeline handle from [initPipeline].
     */
    external fun destroyPipeline(handle: Long)

    /**
     * Validate a config JSON string without creating a pipeline.
     *
     * @param configJson JSON string to validate.
     * @return JSON string: `{"valid": true}` or `{"valid": false, "error": "..."}`.
     */
    external fun validateConfig(configJson: String): String

    /**
     * Test connectivity to model service.
     *
     * @param configJson Full config JSON.
     * @param service service id，当前仅支持 "llm"。
     * @return JSON string: `{"success": true}` or `{"success": false, "error": "..."}`.
     */
    external fun testConnection(configJson: String, service: String): String

    /**
     * Cancel an in-progress streaming session.
     *
     * @param handle Pipeline handle.
     */
    external fun cancelProcessing(handle: Long)

    /**
     * Get the core library version string.
     *
     * @return Version string (e.g. "0.5.0").
     */
    external fun getVersion(): String
}
