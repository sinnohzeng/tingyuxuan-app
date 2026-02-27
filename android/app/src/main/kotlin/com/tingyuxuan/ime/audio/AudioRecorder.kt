package com.tingyuxuan.ime.audio

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import androidx.core.content.ContextCompat
import java.io.File
import java.io.RandomAccessFile
import kotlin.math.sqrt

/**
 * 音频录制器 — 生成 16kHz mono 16-bit WAV 文件。
 *
 * 输出格式与 Rust tingyuxuan-core 一致：
 * - 采样率: 16000 Hz
 * - 声道: 1 (mono)
 * - 位深: 16-bit signed PCM
 *
 * 在录音前会校验 RECORD_AUDIO 权限，未授权时抛出 [SecurityException]。
 */
class AudioRecorder {

    companion object {
        private const val SAMPLE_RATE = 16000
        private const val CHANNEL_CONFIG = AudioFormat.CHANNEL_IN_MONO
        private const val AUDIO_FORMAT = AudioFormat.ENCODING_PCM_16BIT
    }

    private var audioRecord: AudioRecord? = null
    private var recordingThread: Thread? = null
    private var outputPath: String = ""
    @Volatile
    private var isRecording = false
    @Volatile
    private var currentAmplitude: Float = 0f

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
     * 开始录音。录音文件保存在指定的缓存目录。
     *
     * @throws SecurityException 如果 RECORD_AUDIO 权限未授予
     * @throws IllegalStateException 如果已在录音中
     */
    fun start(context: Context, cacheDir: File) {
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
            bufferSize * 2,
        )

        val outputFile = File(cacheDir, "recording_${System.currentTimeMillis()}.wav")
        outputPath = outputFile.absolutePath

        audioRecord = record
        isRecording = true

        recordingThread = Thread {
            writeWav(record, outputFile, bufferSize)
        }.apply { start() }
    }

    /**
     * 停止录音并返回 WAV 文件路径。
     *
     * @throws IllegalStateException 如果未在录音中
     */
    fun stop(): String {
        if (!isRecording) throw IllegalStateException("Not recording")

        isRecording = false
        recordingThread?.join(3000)
        recordingThread = null

        audioRecord?.stop()
        audioRecord?.release()
        audioRecord = null

        return outputPath
    }

    /**
     * 强制取消录音（不返回文件路径）。
     * 如果未在录音中则忽略。
     */
    fun cancel() {
        if (!isRecording) return

        isRecording = false
        recordingThread?.join(3000)
        recordingThread = null

        audioRecord?.stop()
        audioRecord?.release()
        audioRecord = null

        // 删除录音文件
        if (outputPath.isNotEmpty()) {
            File(outputPath).delete()
            outputPath = ""
        }

        currentAmplitude = 0f
    }

    /**
     * 获取当前 RMS 振幅 (0.0 到 1.0)，用于 UI 波形展示。
     */
    fun getAmplitude(): Float = currentAmplitude

    private fun writeWav(record: AudioRecord, file: File, bufferSize: Int) {
        val raf = RandomAccessFile(file, "rw")
        // Write WAV header placeholder (44 bytes).
        writeWavHeader(raf, 0)

        record.startRecording()
        val buffer = ShortArray(bufferSize / 2)
        var totalSamples = 0L

        while (isRecording) {
            val read = record.read(buffer, 0, buffer.size)
            if (read > 0) {
                // Write PCM data as little-endian 16-bit.
                for (i in 0 until read) {
                    val sample = buffer[i]
                    raf.writeByte(sample.toInt() and 0xFF)
                    raf.writeByte((sample.toInt() shr 8) and 0xFF)
                }
                totalSamples += read

                // Compute RMS for volume visualization.
                var sumSquares = 0.0
                for (i in 0 until read) {
                    val normalized = buffer[i].toFloat() / Short.MAX_VALUE
                    sumSquares += normalized * normalized
                }
                currentAmplitude = sqrt(sumSquares / read).toFloat().coerceIn(0f, 1f)
            }
        }

        // Update WAV header with actual data size.
        val dataSize = totalSamples * 2 // 16-bit = 2 bytes per sample
        raf.seek(0)
        writeWavHeader(raf, dataSize)
        raf.close()

        currentAmplitude = 0f
    }

    private fun writeWavHeader(raf: RandomAccessFile, dataSize: Long) {
        val totalSize = 36 + dataSize

        // RIFF header
        raf.writeBytes("RIFF")
        raf.writeIntLE(totalSize.toInt())
        raf.writeBytes("WAVE")

        // fmt subchunk
        raf.writeBytes("fmt ")
        raf.writeIntLE(16)          // Subchunk1Size (PCM)
        raf.writeShortLE(1)         // AudioFormat (PCM = 1)
        raf.writeShortLE(1)         // NumChannels (mono)
        raf.writeIntLE(SAMPLE_RATE) // SampleRate
        raf.writeIntLE(SAMPLE_RATE * 2) // ByteRate
        raf.writeShortLE(2)         // BlockAlign
        raf.writeShortLE(16)        // BitsPerSample

        // data subchunk
        raf.writeBytes("data")
        raf.writeIntLE(dataSize.toInt())
    }

    // Little-endian write helpers.
    private fun RandomAccessFile.writeIntLE(value: Int) {
        writeByte(value and 0xFF)
        writeByte((value shr 8) and 0xFF)
        writeByte((value shr 16) and 0xFF)
        writeByte((value shr 24) and 0xFF)
    }

    private fun RandomAccessFile.writeShortLE(value: Int) {
        writeByte(value and 0xFF)
        writeByte((value shr 8) and 0xFF)
    }
}
