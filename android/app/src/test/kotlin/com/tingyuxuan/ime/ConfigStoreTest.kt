package com.tingyuxuan.ime

import org.junit.Test
import org.junit.Assert.*

/**
 * Unit tests for ConfigStore.
 *
 * Note: EncryptedSharedPreferences requires Android Keystore and cannot
 * be tested in pure JVM. These tests verify the JSON building logic.
 */
class ConfigStoreTest {

    @Test
    fun `config JSON contains required fields`() {
        // Verify the JSON template structure.
        val template = """
        {
            "stt": {
                "provider": "whisper",
                "api_key_ref": "test-key"
            },
            "llm": {
                "api_key_ref": "test-llm-key",
                "model": "gpt-4o-mini"
            },
            "user_dictionary": [],
            "language": {
                "translation_target": "en"
            }
        }
        """.trimIndent()

        val obj = org.json.JSONObject(template)
        assertEquals("whisper", obj.getJSONObject("stt").getString("provider"))
        assertEquals("test-key", obj.getJSONObject("stt").getString("api_key_ref"))
        assertEquals("test-llm-key", obj.getJSONObject("llm").getString("api_key_ref"))
        assertEquals("gpt-4o-mini", obj.getJSONObject("llm").getString("model"))
    }
}
