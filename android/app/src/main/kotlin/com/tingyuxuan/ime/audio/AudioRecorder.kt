package com.tingyuxuan.ime.audio

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import androidx.core.content.ContextCompat
import kotlin.math.sqrt

/**
 * 音频录制器 — 实时输出 16kHz mono 16-bit PCM 帧。
 *
 * 输出格式与 Rust tingyuxuan-core 一致：
 * - 采样率: 16000 Hz
 * - 声道: 1 (mono)
 * - 位深: 16-bit signed PCM
 *
 * 不再写入 WAV 文件，而是通过 [AudioChunkListener] 回调实时发送 PCM 帧，
 * 由调用方转发到 Rust 流式 STT。
 */
class AudioRecorder {

    companion object {
        private const val SAMPLE_RATE = 16000
        private const val CHANNEL_CONFIG = AudioFormat.CHANNEL_IN_MONO
        private const val AUDIO_FORMAT = AudioFormat.ENCODING_PCM_16BIT
    }

    /**
     * 音频帧回调。每 ~20ms 调用一次，传入 PCM16 采样数据。
     */
    fun interface AudioChunkListener {
        fun onChunk(pcmData: ShortArray)
    }

    private var audioRecord: AudioRecord? = null
    private var recordingThread: Thread? = null
    @Volatile
    private var isRecording = false
    @Volatile
    private var currentAmplitude: Float = 0f
    private var chunkListener: AudioChunkListener? = null

    /** 当前是否正在录音 */
    val recording: Boolean get() = isRecording

    /**
     * 检查录音权限是否已授予。
     */
    fun hasPermission(context: Context): Boolean {
        return ContextCompat.checkSelfPermission(
            context, Manifest.permission.RECORD_AUDIO
        ) == PackageManager.PERMISSION_GRANTED
    }

    /**
     * 开始录音。PCM 帧通过 [listener] 实时回调。
     *
     * @throws SecurityException 如果 RECORD_AUDIO 权限未授予
     * @throws IllegalStateException 如果已在录音中
     */
    fun start(context: Context, listener: AudioChunkListener) {
        if (isRecording) throw IllegalStateException("Already recording")
        if (!hasPermission(context)) {
            throw SecurityException("RECORD_AUDIO permission not granted")
        }

        val bufferSize = AudioRecord.getMinBufferSize(SAMPLE_RATE, CHANNEL_CONFIG, AUDIO_FORMAT)
        val record = AudioRecord(
            MediaRecorder.AudioSource.MIC,
            SAMPLE_RATE,
            CHANNEL_CONFIG,
            AUDIO_FORMAT,
            bufferSize * 2, // 双缓冲，降低采集延迟
        )

        audioRecord = record
        chunkListener = listener
        isRecording = true

        recordingThread = Thread {
            streamPcm(record, bufferSize)
        }.apply { start() }
    }

    /**
     * 停止录音。
     *
     * @throws IllegalStateException 如果未在录音中
     */
    fun stop() {
        if (!isRecording) throw IllegalStateException("Not recording")
        cleanup()
    }

    /**
     * 强制取消录音。
     * 如果未在录音中则忽略。
     */
    fun cancel() {
        if (!isRecording) return
        cleanup()
    }

    private fun cleanup() {
        isRecording = false
        // 3s 超时：等待录音线程完成最后一帧的处理和回调
        recordingThread?.join(3000)
        recordingThread = null

        audioRecord?.stop()
        audioRecord?.release()
        audioRecord = null
        chunkListener = null
        resetAmplitude()
    }

    private fun resetAmplitude() {
        currentAmplitude = 0f
    }

    /**
     * 获取当前 RMS 振幅 (0.0 到 1.0)，用于 UI 波形展示。
     */
    fun getAmplitude(): Float = currentAmplitude

    private fun streamPcm(record: AudioRecord, bufferSize: Int) {
        record.startRecording()
        // ~20ms 帧大小 (320 samples @ 16kHz)，对齐 Rust/Opus 要求
        val frameSize = 320
        val buffer = ShortArray(frameSize)

        while (isRecording) {
            val read = record.read(buffer, 0, buffer.size)
            if (read > 0) {
                // 通过回调发送 PCM 帧
                val chunk = buffer.copyOf(read)
                chunkListener?.onChunk(chunk)

                // Compute RMS for volume visualization.
                var sumSquares = 0.0
                for (i in 0 until read) {
                    val normalized = buffer[i].toFloat() / Short.MAX_VALUE
                    sumSquares += normalized * normalized
                }
                currentAmplitude = sqrt(sumSquares / read).toFloat().coerceIn(0f, 1f)
            } else if (read == 0) {
                // 空读取，短暂休眠避免 CPU 空转
                Thread.sleep(1)
            }
        }

        resetAmplitude()
    }
}
