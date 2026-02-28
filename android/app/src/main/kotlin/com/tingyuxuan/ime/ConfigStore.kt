package com.tingyuxuan.ime

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import org.json.JSONArray
import org.json.JSONObject

/**
 * 加密配置存储 — 使用 EncryptedSharedPreferences 保护 API Key。
 *
 * 所有配置项通过类型安全的属性访问，JSON 构建使用 [JSONObject]
 * 而非字符串模板（防止 JSON 注入）。
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

    // --- STT 配置 ---

    var sttProvider: String
        get() = prefs.getString("stt_provider", "dashscope_streaming") ?: "dashscope_streaming"
        set(value) = prefs.edit().putString("stt_provider", value).apply()

    var sttApiKey: String
        get() = prefs.getString("stt_api_key", "") ?: ""
        set(value) = prefs.edit().putString("stt_api_key", value).apply()

    var sttBaseUrl: String?
        get() = prefs.getString("stt_base_url", null)
        set(value) = prefs.edit().putString("stt_base_url", value).apply()

    var sttModel: String?
        get() = prefs.getString("stt_model", null)
        set(value) = prefs.edit().putString("stt_model", value).apply()

    // --- LLM 配置 ---

    var llmProvider: String
        get() = prefs.getString("llm_provider", "openai") ?: "openai"
        set(value) = prefs.edit().putString("llm_provider", value).apply()

    var llmApiKey: String
        get() = prefs.getString("llm_api_key", "") ?: ""
        set(value) = prefs.edit().putString("llm_api_key", value).apply()

    var llmBaseUrl: String?
        get() = prefs.getString("llm_base_url", null)
        set(value) = prefs.edit().putString("llm_base_url", value).apply()

    var llmModel: String
        get() = prefs.getString("llm_model", "gpt-4o-mini") ?: "gpt-4o-mini"
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

    /** API Key 是否已配置（STT + LLM 都有值） */
    val isConfigured: Boolean
        get() = sttApiKey.isNotEmpty() && llmApiKey.isNotEmpty()

    /**
     * 构建符合 tingyuxuan-core AppConfig 格式的 JSON。
     *
     * 使用 [JSONObject] 确保特殊字符正确转义，防止 JSON 注入。
     * 如果必填的 API Key 缺失，返回 null。
     */
    fun buildConfigJson(): String? {
        if (!isConfigured) return null

        val sttObj = JSONObject().apply {
            put("provider", sttProvider)
            put("api_key_ref", sttApiKey)
            sttBaseUrl?.let { put("base_url", it) }
            sttModel?.let { put("model", it) }
        }

        val llmObj = JSONObject().apply {
            put("provider", llmProvider)
            put("api_key_ref", llmApiKey)
            put("model", llmModel)
            llmBaseUrl?.let { put("base_url", it) }
        }

        val languageObj = JSONObject().apply {
            put("translation_target", translationTarget)
        }

        val config = JSONObject().apply {
            put("stt", sttObj)
            put("llm", llmObj)
            put("user_dictionary", JSONArray())
            put("language", languageObj)
        }

        return config.toString()
    }
}
