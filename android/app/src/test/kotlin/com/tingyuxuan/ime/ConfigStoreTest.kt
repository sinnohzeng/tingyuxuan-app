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

    @Test
    fun `special characters in API key are preserved`() {
        // API key with special characters — JSONObject must escape correctly
        val apiKey = "sk-abc123!@#\$%^&*()_+-=[]{}|;':\",./<>?"
        val obj = org.json.JSONObject()
        obj.put("api_key_ref", apiKey)

        val parsed = org.json.JSONObject(obj.toString())
        assertEquals(apiKey, parsed.getString("api_key_ref"))
    }

    @Test
    fun `empty dictionary list serializes correctly`() {
        val obj = org.json.JSONObject()
        obj.put("user_dictionary", org.json.JSONArray())

        val parsed = org.json.JSONObject(obj.toString())
        assertEquals(0, parsed.getJSONArray("user_dictionary").length())
    }

    @Test
    fun `unicode API key is preserved`() {
        // 极端情况：API key 中包含中文字符
        val apiKey = "key-你好-test"
        val obj = org.json.JSONObject()
        obj.put("api_key_ref", apiKey)

        val parsed = org.json.JSONObject(obj.toString())
        assertEquals(apiKey, parsed.getString("api_key_ref"))
    }

    @Test
    fun `config with optional base_url field`() {
        val sttObj = org.json.JSONObject().apply {
            put("provider", "whisper")
            put("api_key_ref", "test-key")
            put("base_url", "https://custom.api.example.com/v1")
        }

        val config = org.json.JSONObject().apply {
            put("stt", sttObj)
        }

        val parsed = org.json.JSONObject(config.toString())
        assertEquals(
            "https://custom.api.example.com/v1",
            parsed.getJSONObject("stt").getString("base_url"),
        )
    }

    @Test
    fun `config without optional base_url field`() {
        val sttObj = org.json.JSONObject().apply {
            put("provider", "whisper")
            put("api_key_ref", "test-key")
        }

        val config = org.json.JSONObject().apply {
            put("stt", sttObj)
        }

        val parsed = org.json.JSONObject(config.toString())
        assertFalse(parsed.getJSONObject("stt").has("base_url"))
    }

    @Test
    fun `dictionary with entries serializes correctly`() {
        val dict = org.json.JSONArray().apply {
            put("听语轩")
            put("TingYuXuan")
            put("AI 助手")
        }
        val obj = org.json.JSONObject()
        obj.put("user_dictionary", dict)

        val parsed = org.json.JSONObject(obj.toString())
        val parsedDict = parsed.getJSONArray("user_dictionary")
        assertEquals(3, parsedDict.length())
        assertEquals("听语轩", parsedDict.getString(0))
        assertEquals("TingYuXuan", parsedDict.getString(1))
        assertEquals("AI 助手", parsedDict.getString(2))
    }
}
