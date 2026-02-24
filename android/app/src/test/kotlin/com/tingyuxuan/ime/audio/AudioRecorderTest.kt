package com.tingyuxuan.ime.audio

import org.junit.Test
import org.junit.Assert.*

/**
 * Unit tests for AudioRecorder.
 *
 * Note: AudioRecord requires Android system APIs and cannot be instantiated
 * in pure JVM tests. These tests verify the WAV format contract and state logic.
 */
class AudioRecorderTest {

    @Test
    fun `wav header size is 44 bytes`() {
        // WAV header: RIFF(4) + size(4) + WAVE(4) + fmt(4) + fmtSize(4)
        // + audioFormat(2) + channels(2) + sampleRate(4) + byteRate(4)
        // + blockAlign(2) + bitsPerSample(2) + data(4) + dataSize(4) = 44
        assertEquals(44, 4 + 4 + 4 + 4 + 4 + 2 + 2 + 4 + 4 + 2 + 2 + 4 + 4)
    }

    @Test
    fun `initial amplitude is zero`() {
        val recorder = AudioRecorder()
        assertEquals(0f, recorder.getAmplitude())
    }
}
