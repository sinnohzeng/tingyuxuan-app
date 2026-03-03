package com.tingyuxuan.ime

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.ExposedDropdownMenu
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.tingyuxuan.core.NativeCore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject

/**
 * 设置页面：单步多模态配置（仅 LLM 必填）+ 语言参数。
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

private val LLM_PROVIDERS = listOf(
    "dashscope" to "DashScope (通义千问)",
    "openai" to "OpenAI",
    "volcengine" to "Volcengine (豆包)",
    "custom" to "自定义",
)

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
    var llmProvider by remember { mutableStateOf(configStore.llmProvider) }
    var llmApiKey by remember { mutableStateOf(configStore.llmApiKey) }
    var llmBaseUrl by remember { mutableStateOf(configStore.llmBaseUrl ?: "") }
    var llmModel by remember { mutableStateOf(configStore.llmModel) }
    var translationTarget by remember { mutableStateOf(configStore.translationTarget) }

    var saved by remember { mutableStateOf(false) }
    var testResult by remember { mutableStateOf<String?>(null) }
    var testing by remember { mutableStateOf(false) }
    val coroutineScope = rememberCoroutineScope()

    fun saveAll() {
        configStore.llmProvider = llmProvider
        configStore.llmApiKey = llmApiKey
        configStore.llmBaseUrl = llmBaseUrl.ifEmpty { null }
        configStore.llmModel = llmModel
        configStore.translationTarget = translationTarget
        saved = true
    }

    fun testLlmConnection() {
        if (!NativeCore.isLoaded) {
            testResult = "核心库未加载"
            return
        }
        saveAll()
        val configJson = configStore.buildConfigJson()
        if (configJson == null) {
            testResult = "请先填写 LLM API Key"
            return
        }
        testing = true
        coroutineScope.launch {
            val result = withContext(Dispatchers.IO) {
                try {
                    NativeCore.testConnection(configJson, "llm")
                } catch (e: Exception) {
                    """{"success": false, "message": "${e.message}"}"""
                }
            }
            testing = false
            testResult = parseConnectionResult(result)
        }
    }

    Scaffold(
        topBar = { TopAppBar(title = { Text("听语轩 设置") }) },
    ) { padding ->
        Column(
            modifier = Modifier
                .padding(padding)
                .padding(16.dp)
                .fillMaxSize()
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
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

            OutlinedButton(
                onClick = ::testLlmConnection,
                enabled = !testing && llmApiKey.isNotEmpty(),
            ) {
                Text(if (testing) "测试中..." else "测试 LLM 连接")
            }
            testResult?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (it.contains("\u2713")) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        MaterialTheme.colorScheme.error
                    },
                )
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 4.dp))

            SectionHeader("语言设置")

            ProviderSelector(
                label = "翻译目标语言",
                providers = TRANSLATION_LANGUAGES,
                selected = translationTarget,
                onSelected = { translationTarget = it },
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 4.dp))

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

            SectionHeader("关于")
            val coreVersion = remember {
                if (NativeCore.isLoaded) {
                    try {
                        NativeCore.getVersion()
                    } catch (_: Exception) {
                        "未知"
                    }
                } else {
                    "核心库未加载"
                }
            }
            Text(
                "应用版本: 0.10.3\n核心库版本: $coreVersion",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            Spacer(modifier = Modifier.height(32.dp))
        }
    }
}

private fun parseConnectionResult(result: String): String {
    return try {
        val json = JSONObject(result)
        if (json.optBoolean("success", false)) {
            "连接成功 \u2713"
        } else {
            "连接失败: ${json.optString("message", "未知错误")}"
        }
    } catch (e: Exception) {
        "解析失败: ${e.message}"
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
