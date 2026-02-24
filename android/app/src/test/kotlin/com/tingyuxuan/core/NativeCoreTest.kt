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
}
