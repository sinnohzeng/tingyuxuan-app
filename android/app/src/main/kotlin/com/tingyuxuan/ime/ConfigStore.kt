package com.tingyuxuan.ime

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

/**
 * Encrypted configuration store for API keys and settings.
 *
 * Uses EncryptedSharedPreferences (AndroidX Security) to protect API keys at rest.
 */
class ConfigStore(context: Context) {

    private val masterKey = MasterKey.Builder(context)
        .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
        .build()

    private val prefs: SharedPreferences = EncryptedSharedPreferences.create(
        context,
        "tingyuxuan_config",
        masterKey,
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )

    var sttProvider: String
        get() = prefs.getString("stt_provider", "whisper") ?: "whisper"
        set(value) = prefs.edit().putString("stt_provider", value).apply()

    var sttApiKey: String
        get() = prefs.getString("stt_api_key", "") ?: ""
        set(value) = prefs.edit().putString("stt_api_key", value).apply()

    var sttBaseUrl: String?
        get() = prefs.getString("stt_base_url", null)
        set(value) = prefs.edit().putString("stt_base_url", value).apply()

    var llmApiKey: String
        get() = prefs.getString("llm_api_key", "") ?: ""
        set(value) = prefs.edit().putString("llm_api_key", value).apply()

    var llmBaseUrl: String?
        get() = prefs.getString("llm_base_url", null)
        set(value) = prefs.edit().putString("llm_base_url", value).apply()

    var llmModel: String
        get() = prefs.getString("llm_model", "gpt-4o-mini") ?: "gpt-4o-mini"
        set(value) = prefs.edit().putString("llm_model", value).apply()

    /**
     * Build a config JSON string matching tingyuxuan-core's AppConfig schema.
     * Returns null if required keys are missing.
     */
    fun buildConfigJson(): String? {
        if (sttApiKey.isEmpty() || llmApiKey.isEmpty()) return null

        return """
        {
            "stt": {
                "provider": "$sttProvider",
                "api_key_ref": "$sttApiKey"
                ${sttBaseUrl?.let { """, "base_url": "$it"""" } ?: ""}
            },
            "llm": {
                "api_key_ref": "$llmApiKey",
                "model": "$llmModel"
                ${llmBaseUrl?.let { """, "base_url": "$it"""" } ?: ""}
            },
            "user_dictionary": [],
            "language": {
                "translation_target": "en"
            }
        }
        """.trimIndent()
    }
}
