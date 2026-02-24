package com.tingyuxuan.ime

import org.junit.Test
import org.junit.Assert.*

/**
 * Unit tests for TingYuXuanIMEService logic.
 *
 * Note: InputMethodService requires Android framework and cannot be
 * instantiated in pure JVM tests. These tests verify helper logic.
 */
class IMEServiceTest {

    @Test
    fun `parse success result extracts text`() {
        val json = """{"success": true, "text": "你好世界"}"""
        val obj = org.json.JSONObject(json)
        assertTrue(obj.getBoolean("success"))
        assertEquals("你好世界", obj.getString("text"))
    }

    @Test
    fun `parse error result contains error message`() {
        val json = """{"success": false, "error": "Network timeout"}"""
        val obj = org.json.JSONObject(json)
        assertFalse(obj.getBoolean("success"))
        assertEquals("Network timeout", obj.getString("error"))
    }

    @Test
    fun `keyboard actions are distinct`() {
        val start = KeyboardAction.StartRecording("dictate")
        val stop = KeyboardAction.StopRecording
        val switch = KeyboardAction.SwitchMode("translate")

        assertNotEquals(start, stop)
        assertNotEquals(start, switch)
    }
}
