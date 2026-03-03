package com.tingyuxuan.ime

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import org.json.JSONArray
import org.json.JSONObject

/**
 * 加密配置存储。
 *
 * 使用 EncryptedSharedPreferences 保护 API Key。
 * 当前架构只要求 LLM 配置；历史 STT 字段仅用于读取兼容迁移，不再写入配置 JSON。
 */
class ConfigStore(context: Context) {

    companion object {
        @Volatile
        private var masterKeyInstance: MasterKey? = null

        private fun getMasterKey(context: Context): MasterKey {
            return masterKeyInstance ?: synchronized(this) {
                masterKeyInstance ?: MasterKey.Builder(context.applicationContext)
                    .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
                    .build()
                    .also { masterKeyInstance = it }
            }
        }
    }

    private val prefs: SharedPreferences = EncryptedSharedPreferences.create(
        context,
        "tingyuxuan_config",
        getMasterKey(context),
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )

    // --- LLM 配置 ---

    var llmProvider: String
        get() = prefs.getString("llm_provider", "dashscope") ?: "dashscope"
        set(value) = prefs.edit().putString("llm_provider", value).apply()

    var llmApiKey: String
        get() = prefs.getString("llm_api_key", "") ?: ""
        set(value) = prefs.edit().putString("llm_api_key", value).apply()

    var llmBaseUrl: String?
        get() = prefs.getString("llm_base_url", null)
        set(value) = prefs.edit().putString("llm_base_url", value).apply()

    var llmModel: String
        get() = prefs.getString("llm_model", "qwen3-omni-flash") ?: "qwen3-omni-flash"
        set(value) = prefs.edit().putString("llm_model", value).apply()

    // --- 语言配置 ---

    var translationTarget: String
        get() = prefs.getString("translation_target", "en") ?: "en"
        set(value) = prefs.edit().putString("translation_target", value).apply()

    var primaryLanguage: String
        get() = prefs.getString("primary_language", "zh") ?: "zh"
        set(value) = prefs.edit().putString("primary_language", value).apply()

    // --- Onboarding 状态 ---

    var onboardingCompleted: Boolean
        get() = prefs.getBoolean("onboarding_completed", false)
        set(value) = prefs.edit().putBoolean("onboarding_completed", value).apply()

    init {
        migrateLegacySttConfigIfNeeded()
    }

    /** API Key 是否已配置（仅 LLM 必填） */
    val isConfigured: Boolean
        get() = llmApiKey.isNotEmpty()

    /**
     * 构建符合 tingyuxuan-core AppConfig 格式的 JSON。
     *
     * 使用 [JSONObject] 确保特殊字符正确转义，防止 JSON 注入。
     * 如果必填的 API Key 缺失，返回 null。
     */
    fun buildConfigJson(): String? {
        if (!isConfigured) return null

        val llmObj = JSONObject().apply {
            put("provider", llmProvider)
            put("api_key_ref", llmApiKey)
            put("model", llmModel)
            llmBaseUrl?.let { put("base_url", it) }
        }

        val languageObj = JSONObject().apply {
            put("primary", primaryLanguage)
            put("translation_target", translationTarget)
        }

        val config = JSONObject().apply {
            put("config_version", 2)
            put("llm", llmObj)
            put("user_dictionary", JSONArray())
            put("language", languageObj)
        }

        return config.toString()
    }

    private fun migrateLegacySttConfigIfNeeded() {
        if (llmApiKey.isNotEmpty()) {
            return
        }
        val legacyApiKey = prefs.getString("stt_api_key", "") ?: ""
        if (legacyApiKey.isEmpty()) {
            return
        }

        val editor = prefs.edit()
        editor.putString("llm_api_key", legacyApiKey)
        putIfNotEmpty(editor, "llm_base_url", prefs.getString("stt_base_url", null))
        putIfNotEmpty(editor, "llm_model", prefs.getString("stt_model", null))
        editor.apply()
    }

    private fun putIfNotEmpty(editor: SharedPreferences.Editor, key: String, value: String?) {
        if (!value.isNullOrEmpty()) {
            editor.putString(key, value)
        }
    }
}
