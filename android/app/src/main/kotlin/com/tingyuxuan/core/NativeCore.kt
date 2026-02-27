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
     * Process an audio file through the STT → LLM pipeline.
     *
     * @param handle Pipeline handle from [initPipeline].
     * @param audioPath Absolute path to the 16kHz mono WAV file.
     * @param mode Processing mode: "dictate", "translate", "edit", "ai_assistant".
     * @param selectedText Optional selected text for Edit mode (empty string if none).
     * @return JSON string with structured result.
     */
    external fun processAudio(
        handle: Long,
        audioPath: String,
        mode: String,
        selectedText: String,
    ): String

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
     * Test connectivity to STT or LLM service.
     *
     * @param configJson Full config JSON.
     * @param service "stt" or "llm".
     * @return JSON string: `{"success": true}` or `{"success": false, "error": "..."}`.
     */
    external fun testConnection(configJson: String, service: String): String

    /**
     * Cancel an in-progress audio processing task.
     *
     * @param handle Pipeline handle.
     */
    external fun cancelProcessing(handle: Long)

    /**
     * Get the core library version string.
     *
     * @return Version string (e.g. "0.4.0").
     */
    external fun getVersion(): String
}
