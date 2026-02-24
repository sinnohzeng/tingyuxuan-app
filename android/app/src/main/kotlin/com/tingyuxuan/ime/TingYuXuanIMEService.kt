package com.tingyuxuan.ime

import android.inputmethodservice.InputMethodService
import android.view.View
import com.tingyuxuan.core.NativeCore
import com.tingyuxuan.ime.audio.AudioRecorder
import com.tingyuxuan.ime.ui.TingYuXuanKeyboard
import kotlinx.coroutines.*

/**
 * TingYuXuan Input Method Service.
 *
 * Lifecycle:
 * 1. onCreate: Initialize pipeline via NativeCore
 * 2. onCreateInputView: Show keyboard UI (Compose)
 * 3. Recording: AudioRecorder captures 16kHz WAV
 * 4. Processing: NativeCore.processAudio() runs STT → LLM
 * 5. Output: currentInputConnection.commitText() injects result
 * 6. onDestroy: Clean up pipeline handle
 */
class TingYuXuanIMEService : InputMethodService() {

    private var pipelineHandle: Long = 0L
    private val recorder = AudioRecorder()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private var currentMode: String = "dictate"
    private var keyboardView: View? = null

    override fun onCreate() {
        super.onCreate()
        initializePipeline()
    }

    override fun onCreateInputView(): View {
        val view = TingYuXuanKeyboard.createView(this) { action ->
            when (action) {
                is KeyboardAction.StartRecording -> startRecording(action.mode)
                is KeyboardAction.StopRecording -> stopRecording()
                is KeyboardAction.SwitchMode -> currentMode = action.mode
            }
        }
        keyboardView = view
        return view
    }

    private fun initializePipeline() {
        val configStore = ConfigStore(this)
        val configJson = configStore.buildConfigJson()
        if (configJson != null) {
            pipelineHandle = NativeCore.initPipeline(configJson)
            if (pipelineHandle == 0L) {
                android.util.Log.e(TAG, "Failed to initialize pipeline")
            }
        }
    }

    fun startRecording(mode: String) {
        currentMode = mode
        try {
            recorder.start(cacheDir)
        } catch (e: Exception) {
            android.util.Log.e(TAG, "Failed to start recording", e)
        }
    }

    fun stopRecording() {
        val audioPath = try {
            recorder.stop()
        } catch (e: Exception) {
            android.util.Log.e(TAG, "Failed to stop recording", e)
            return
        }

        if (pipelineHandle == 0L) {
            android.util.Log.e(TAG, "Pipeline not initialized")
            return
        }

        scope.launch {
            val resultJson = NativeCore.processAudio(
                pipelineHandle,
                audioPath,
                currentMode,
                ""  // no selected text on Android IME
            )
            val text = parseResult(resultJson)
            if (text != null) {
                withContext(Dispatchers.Main) {
                    currentInputConnection?.commitText(text, 1)
                }
            }
        }
    }

    /**
     * Returns the current RMS amplitude (0.0–1.0) for UI visualization.
     */
    fun getAmplitude(): Float = recorder.getAmplitude()

    private fun parseResult(json: String): String? {
        return try {
            val obj = org.json.JSONObject(json)
            if (obj.getBoolean("success")) {
                obj.getString("text")
            } else {
                android.util.Log.e(TAG, "Pipeline error: ${obj.getString("error")}")
                null
            }
        } catch (e: Exception) {
            android.util.Log.e(TAG, "Failed to parse result JSON", e)
            null
        }
    }

    override fun onDestroy() {
        scope.cancel()
        if (pipelineHandle != 0L) {
            NativeCore.destroyPipeline(pipelineHandle)
            pipelineHandle = 0L
        }
        super.onDestroy()
    }

    companion object {
        private const val TAG = "TingYuXuanIME"
    }
}

/** Actions dispatched from the keyboard UI. */
sealed class KeyboardAction {
    data class StartRecording(val mode: String) : KeyboardAction()
    data object StopRecording : KeyboardAction()
    data class SwitchMode(val mode: String) : KeyboardAction()
}
