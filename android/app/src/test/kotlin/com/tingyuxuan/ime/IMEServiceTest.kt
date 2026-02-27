package com.tingyuxuan.ime

import com.tingyuxuan.ime.model.*
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
    fun `parse structured error result`() {
        val json = """{"success": false, "error_code": "stt_auth_failed", "message": "API key invalid", "user_action": "check_api_key"}"""
        val obj = org.json.JSONObject(json)
        assertFalse(obj.getBoolean("success"))
        assertEquals("stt_auth_failed", obj.getString("error_code"))
        assertEquals("API key invalid", obj.getString("message"))
        assertEquals("check_api_key", obj.getString("user_action"))
    }

    @Test
    fun `keyboard intents are distinct`() {
        val start = KeyboardIntent.StartRecording(ProcessingMode.Dictate)
        val stop = KeyboardIntent.StopRecording
        val switch = KeyboardIntent.SwitchMode(ProcessingMode.Translate)

        assertNotEquals(start, stop)
        assertNotEquals(start, switch)
    }

    @Test
    fun `processing modes have correct ids`() {
        assertEquals("dictate", ProcessingMode.Dictate.id)
        assertEquals("translate", ProcessingMode.Translate.id)
        assertEquals("ai_assistant", ProcessingMode.AiAssistant.id)
        assertEquals("edit", ProcessingMode.Edit.id)
    }

    @Test
    fun `error codes map to user actions`() {
        assertEquals(UserAction.RequestPermission, ErrorCode.PermissionDenied.userAction)
        assertEquals(UserAction.OpenSettings, ErrorCode.ApiKeyMissing.userAction)
        assertEquals(UserAction.Retry, ErrorCode.NetworkError.userAction)
        assertEquals(UserAction.CheckApiKey, ErrorCode.SttAuthFailed.userAction)
        assertEquals(UserAction.CheckApiKey, ErrorCode.LlmAuthFailed.userAction)
        assertEquals(UserAction.Retry, ErrorCode.Timeout.userAction)
        assertEquals(UserAction.Reinstall, ErrorCode.NativeLibraryMissing.userAction)
    }

    @Test
    fun `IME states are sealed correctly`() {
        val idle: IMEState = IMEState.Idle()
        val recording: IMEState = IMEState.Recording(mode = ProcessingMode.Dictate)
        val processing: IMEState = IMEState.Processing(mode = ProcessingMode.Dictate)
        val done: IMEState = IMEState.Done(text = "test", mode = ProcessingMode.Dictate)
        val error: IMEState = IMEState.Error(ErrorCode.NetworkError, "message")

        assertTrue(idle is IMEState.Idle)
        assertTrue(recording is IMEState.Recording)
        assertTrue(processing is IMEState.Processing)
        assertTrue(done is IMEState.Done)
        assertTrue(error is IMEState.Error)
    }
}
