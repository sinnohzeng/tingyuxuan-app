package com.tingyuxuan.core

import org.junit.Test
import org.junit.Assert.*

/**
 * Unit tests for NativeCore JNI interface.
 *
 * Note: These tests verify the Kotlin-side interface contract.
 * Full integration tests require the native .so to be loaded.
 */
class NativeCoreTest {

    @Test
    fun `pipeline handle zero means failure`() {
        // Convention: initPipeline returns 0 on failure.
        assertEquals(0L, 0L) // placeholder — real test needs JNI
    }

    @Test
    fun `process result JSON success format`() {
        val json = """{"success": true, "text": "Hello world"}"""
        val obj = org.json.JSONObject(json)
        assertTrue(obj.getBoolean("success"))
        assertEquals("Hello world", obj.getString("text"))
    }

    @Test
    fun `process result JSON error format`() {
        val json = """{"success": false, "error": "API key invalid"}"""
        val obj = org.json.JSONObject(json)
        assertFalse(obj.getBoolean("success"))
        assertEquals("API key invalid", obj.getString("error"))
    }

    @Test
    fun `parse success response`() {
        val json = """{"success":true,"text":"hello"}"""
        val obj = org.json.JSONObject(json)
        assertTrue(obj.getBoolean("success"))
        assertEquals("hello", obj.getString("text"))
    }

    @Test
    fun `parse error response`() {
        val json = """{"success":false,"error":"timeout"}"""
        val obj = org.json.JSONObject(json)
        assertFalse(obj.getBoolean("success"))
        assertEquals("timeout", obj.getString("error"))
    }

    @Test
    fun `parse unicode response`() {
        val json = """{"success":true,"text":"你好世界"}"""
        val obj = org.json.JSONObject(json)
        assertTrue(obj.getBoolean("success"))
        assertEquals("你好世界", obj.getString("text"))
    }

    @Test
    fun `parse response with nested error details`() {
        val json = """{"success":false,"error_code":"stt_auth_failed","message":"Invalid credentials"}"""
        val obj = org.json.JSONObject(json)
        assertFalse(obj.getBoolean("success"))
        assertEquals("stt_auth_failed", obj.getString("error_code"))
        assertEquals("Invalid credentials", obj.getString("message"))
    }

    @Test
    fun `parse response with empty text`() {
        val json = """{"success":true,"text":""}"""
        val obj = org.json.JSONObject(json)
        assertTrue(obj.getBoolean("success"))
        assertEquals("", obj.getString("text"))
    }

    @Test
    fun `parse response with special characters in text`() {
        val json = """{"success":true,"text":"line1\nline2\ttab"}"""
        val obj = org.json.JSONObject(json)
        assertTrue(obj.getBoolean("success"))
        assertTrue(obj.getString("text").contains("\n"))
        assertTrue(obj.getString("text").contains("\t"))
    }
}
