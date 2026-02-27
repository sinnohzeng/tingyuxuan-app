package com.tingyuxuan.ime

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.tingyuxuan.core.NativeCore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject

/**
 * 设置页面 — 完整的 API 配置、连接测试、语言设置。
 *
 * 从 Onboarding 或键盘设置按钮跳转进入。
 */
class SettingsActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val configStore = ConfigStore(this)

        setContent {
            MaterialTheme {
                SettingsScreen(configStore)
            }
        }
    }
}

// --- STT 供应商选项 ---
private val STT_PROVIDERS = listOf(
    "whisper" to "OpenAI Whisper",
    "dashscope_asr" to "DashScope ASR (阿里云)",
    "custom" to "自定义",
)

// --- LLM 供应商选项 ---
private val LLM_PROVIDERS = listOf(
    "openai" to "OpenAI",
    "dashscope" to "DashScope (通义千问)",
    "volcengine" to "Volcengine (豆包)",
    "custom" to "自定义",
)

// --- 翻译目标语言 ---
private val TRANSLATION_LANGUAGES = listOf(
    "en" to "English",
    "zh" to "中文",
    "ja" to "日本語",
    "ko" to "한국어",
    "es" to "Español",
    "fr" to "Français",
    "de" to "Deutsch",
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsScreen(configStore: ConfigStore) {
    // STT 配置
    var sttProvider by remember { mutableStateOf(configStore.sttProvider) }
    var sttApiKey by remember { mutableStateOf(configStore.sttApiKey) }
    var sttBaseUrl by remember { mutableStateOf(configStore.sttBaseUrl ?: "") }
    var sttModel by remember { mutableStateOf(configStore.sttModel ?: "") }

    // LLM 配置
    var llmProvider by remember { mutableStateOf(configStore.llmProvider) }
    var llmApiKey by remember { mutableStateOf(configStore.llmApiKey) }
    var llmBaseUrl by remember { mutableStateOf(configStore.llmBaseUrl ?: "") }
    var llmModel by remember { mutableStateOf(configStore.llmModel) }

    // 语言配置
    var translationTarget by remember { mutableStateOf(configStore.translationTarget) }

    // UI 状态
    var saved by remember { mutableStateOf(false) }
    var sttTestResult by remember { mutableStateOf<String?>(null) }
    var llmTestResult by remember { mutableStateOf<String?>(null) }
    var testing by remember { mutableStateOf(false) }
    val coroutineScope = rememberCoroutineScope()

    // 保存所有设置
    fun saveAll() {
        configStore.sttProvider = sttProvider
        configStore.sttApiKey = sttApiKey
        configStore.sttBaseUrl = sttBaseUrl.ifEmpty { null }
        configStore.sttModel = sttModel.ifEmpty { null }
        configStore.llmProvider = llmProvider
        configStore.llmApiKey = llmApiKey
        configStore.llmBaseUrl = llmBaseUrl.ifEmpty { null }
        configStore.llmModel = llmModel
        configStore.translationTarget = translationTarget
        saved = true
    }

    // 测试连接
    fun testConnection(service: String, onResult: (String) -> Unit) {
        if (!NativeCore.isLoaded) {
            onResult("核心库未加载")
            return
        }
        // 先保存再测试
        saveAll()
        val configJson = configStore.buildConfigJson()
        if (configJson == null) {
            onResult("请先填写 API Key")
            return
        }
        testing = true
        coroutineScope.launch {
            val result = withContext(Dispatchers.IO) {
                try {
                    NativeCore.testConnection(configJson, service)
                } catch (e: Exception) {
                    """{"success": false, "error": "${e.message}"}"""
                }
            }
            testing = false
            try {
                val json = JSONObject(result)
                if (json.optBoolean("success", false)) {
                    onResult("连接成功 \u2713")
                } else {
                    onResult("连接失败: ${json.optString("error", "未知错误")}")
                }
            } catch (e: Exception) {
                onResult("解析失败: ${e.message}")
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(title = { Text("听语轩 设置") })
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .padding(padding)
                .padding(16.dp)
                .fillMaxSize()
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            // === STT 配置 ===
            SectionHeader("语音识别 (STT)")

            ProviderSelector(
                label = "STT 供应商",
                providers = STT_PROVIDERS,
                selected = sttProvider,
                onSelected = { sttProvider = it },
            )

            OutlinedTextField(
                value = sttApiKey,
                onValueChange = { sttApiKey = it },
                label = { Text("STT API Key") },
                visualTransformation = PasswordVisualTransformation(),
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )

            OutlinedTextField(
                value = sttBaseUrl,
                onValueChange = { sttBaseUrl = it },
                label = { Text("STT Base URL（可选）") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )

            OutlinedTextField(
                value = sttModel,
                onValueChange = { sttModel = it },
                label = { Text("STT 模型（可选）") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )

            // STT 连接测试
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(
                    onClick = { testConnection("stt") { sttTestResult = it } },
                    enabled = !testing && sttApiKey.isNotEmpty(),
                ) {
                    Text(if (testing) "测试中..." else "测试 STT 连接")
                }
                sttTestResult?.let {
                    Text(
                        it,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (it.contains("\u2713")) {
                            MaterialTheme.colorScheme.primary
                        } else {
                            MaterialTheme.colorScheme.error
                        },
                        modifier = Modifier.weight(1f),
                    )
                }
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 4.dp))

            // === LLM 配置 ===
            SectionHeader("语言模型 (LLM)")

            ProviderSelector(
                label = "LLM 供应商",
                providers = LLM_PROVIDERS,
                selected = llmProvider,
                onSelected = { llmProvider = it },
            )

            OutlinedTextField(
                value = llmApiKey,
                onValueChange = { llmApiKey = it },
                label = { Text("LLM API Key") },
                visualTransformation = PasswordVisualTransformation(),
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )

            OutlinedTextField(
                value = llmBaseUrl,
                onValueChange = { llmBaseUrl = it },
                label = { Text("LLM Base URL（可选）") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )

            OutlinedTextField(
                value = llmModel,
                onValueChange = { llmModel = it },
                label = { Text("LLM 模型") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )

            // LLM 连接测试
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(
                    onClick = { testConnection("llm") { llmTestResult = it } },
                    enabled = !testing && llmApiKey.isNotEmpty(),
                ) {
                    Text(if (testing) "测试中..." else "测试 LLM 连接")
                }
                llmTestResult?.let {
                    Text(
                        it,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (it.contains("\u2713")) {
                            MaterialTheme.colorScheme.primary
                        } else {
                            MaterialTheme.colorScheme.error
                        },
                        modifier = Modifier.weight(1f),
                    )
                }
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 4.dp))

            // === 语言设置 ===
            SectionHeader("语言设置")

            ProviderSelector(
                label = "翻译目标语言",
                providers = TRANSLATION_LANGUAGES,
                selected = translationTarget,
                onSelected = { translationTarget = it },
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 4.dp))

            // === 保存按钮 ===
            Button(
                onClick = { saveAll() },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("保存设置")
            }

            if (saved) {
                Text(
                    "设置已保存",
                    color = MaterialTheme.colorScheme.primary,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 4.dp))

            // === 关于 ===
            SectionHeader("关于")

            val coreVersion = remember {
                if (NativeCore.isLoaded) {
                    try { NativeCore.getVersion() } catch (_: Exception) { "未知" }
                } else "核心库未加载"
            }

            Text(
                "应用版本: 0.4.0\n核心库版本: $coreVersion",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            Spacer(modifier = Modifier.height(32.dp))
        }
    }
}

@Composable
private fun SectionHeader(title: String) {
    Text(
        text = title,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.primary,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ProviderSelector(
    label: String,
    providers: List<Pair<String, String>>,
    selected: String,
    onSelected: (String) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    val selectedLabel = providers.find { it.first == selected }?.second ?: selected

    ExposedDropdownMenuBox(
        expanded = expanded,
        onExpandedChange = { expanded = it },
    ) {
        OutlinedTextField(
            value = selectedLabel,
            onValueChange = {},
            readOnly = true,
            label = { Text(label) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            modifier = Modifier
                .fillMaxWidth()
                .menuAnchor(),
        )
        ExposedDropdownMenu(
            expanded = expanded,
            onDismissRequest = { expanded = false },
        ) {
            providers.forEach { (id, displayName) ->
                DropdownMenuItem(
                    text = { Text(displayName) },
                    onClick = {
                        onSelected(id)
                        expanded = false
                    },
                )
            }
        }
    }
}
