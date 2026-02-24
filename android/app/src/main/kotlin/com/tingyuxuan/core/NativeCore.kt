package com.tingyuxuan.core

/**
 * JNI interface to the Rust tingyuxuan-core engine.
 *
 * All methods are blocking — call from a coroutine dispatcher (Dispatchers.IO).
 */
object NativeCore {

    init {
        System.loadLibrary("tingyuxuan_jni")
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
     * @return JSON string: `{"success": true, "text": "..."}` or `{"success": false, "error": "..."}`.
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
}
