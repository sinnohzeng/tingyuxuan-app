package com.tingyuxuan.ime.controller

import android.content.Context
import android.util.Log
import com.tingyuxuan.ime.audio.AudioRecorder
import com.tingyuxuan.ime.model.ErrorCode
import com.tingyuxuan.ime.model.ProcessingMode

/**
 * 录音控制器 — 封装 AudioRecorder，管理录音生命周期。
 *
 * 职责：
 * - 权限检查后启动录音
 * - 停止录音并返回文件路径
 * - 取消录音
 * - 提供振幅数据给 UI
 */
class RecordingController(private val context: Context) {

    companion object {
        private const val TAG = "RecordingController"
    }

    private val recorder = AudioRecorder()

    /** 当前是否正在录音 */
    val isRecording: Boolean get() = recorder.recording

    /** 当前 RMS 振幅 (0.0 ~ 1.0) */
    val amplitude: Float get() = recorder.getAmplitude()

    /**
     * 开始录音。
     *
     * @return null 表示成功，非 null 表示错误码
     */
    fun start(mode: ProcessingMode): ErrorCode? {
        if (isRecording) {
            Log.w(TAG, "Already recording, ignoring start request")
            return null
        }

        if (!recorder.hasPermission(context)) {
            Log.e(TAG, "RECORD_AUDIO permission not granted")
            return ErrorCode.PermissionDenied
        }

        return try {
            recorder.start(context, context.cacheDir)
            Log.i(TAG, "Recording started (mode=${mode.id})")
            null
        } catch (e: SecurityException) {
            Log.e(TAG, "Permission denied", e)
            ErrorCode.PermissionDenied
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start recording", e)
            ErrorCode.Unknown
        }
    }

    /**
     * 停止录音并返回 WAV 文件路径。
     *
     * @return 文件路径，失败时返回 null
     */
    fun stop(): String? {
        if (!isRecording) {
            Log.w(TAG, "Not recording, ignoring stop request")
            return null
        }

        return try {
            val path = recorder.stop()
            Log.i(TAG, "Recording stopped: $path")
            path
        } catch (e: Exception) {
            Log.e(TAG, "Failed to stop recording", e)
            null
        }
    }

    /**
     * 取消录音（不处理录音结果）。
     */
    fun cancel() {
        if (!isRecording) return

        try {
            recorder.cancel()
            Log.i(TAG, "Recording cancelled")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to cancel recording", e)
        }
    }
}
