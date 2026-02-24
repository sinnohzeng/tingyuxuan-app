package com.tingyuxuan.ime.audio

import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import java.io.File
import java.io.RandomAccessFile
import kotlin.math.sqrt

/**
 * Audio recorder that captures 16kHz mono 16-bit WAV files.
 *
 * Outputs WAV format matching the Rust tingyuxuan-core expectations:
 * - Sample rate: 16000 Hz
 * - Channels: 1 (mono)
 * - Bit depth: 16-bit signed PCM
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

    /**
     * Start recording audio to a WAV file in the given cache directory.
     */
    fun start(cacheDir: File) {
        if (isRecording) throw IllegalStateException("Already recording")

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
     * Stop recording and return the path to the WAV file.
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
     * Get the current RMS amplitude (0.0 to 1.0) for UI visualization.
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
        raf.writeIntLE(SAMPLE_RATE * 2) // ByteRate (SampleRate * NumChannels * BitsPerSample/8)
        raf.writeShortLE(2)         // BlockAlign (NumChannels * BitsPerSample/8)
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
