package com.tingyuxuan.ime

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp

/**
 * Settings activity for configuring API keys and provider options.
 *
 * This is launched from the system IME settings or the app launcher icon.
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsScreen(configStore: ConfigStore) {
    var sttApiKey by remember { mutableStateOf(configStore.sttApiKey) }
    var llmApiKey by remember { mutableStateOf(configStore.llmApiKey) }
    var llmModel by remember { mutableStateOf(configStore.llmModel) }
    var saved by remember { mutableStateOf(false) }

    Scaffold(
        topBar = {
            TopAppBar(title = { Text("听语轩 设置") })
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .padding(padding)
                .padding(16.dp)
                .fillMaxSize(),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text("STT 配置", style = MaterialTheme.typography.titleMedium)

            OutlinedTextField(
                value = sttApiKey,
                onValueChange = { sttApiKey = it },
                label = { Text("STT API Key") },
                visualTransformation = PasswordVisualTransformation(),
                modifier = Modifier.fillMaxWidth(),
            )

            Text("LLM 配置", style = MaterialTheme.typography.titleMedium)

            OutlinedTextField(
                value = llmApiKey,
                onValueChange = { llmApiKey = it },
                label = { Text("LLM API Key") },
                visualTransformation = PasswordVisualTransformation(),
                modifier = Modifier.fillMaxWidth(),
            )

            OutlinedTextField(
                value = llmModel,
                onValueChange = { llmModel = it },
                label = { Text("LLM Model") },
                modifier = Modifier.fillMaxWidth(),
            )

            Button(
                onClick = {
                    configStore.sttApiKey = sttApiKey
                    configStore.llmApiKey = llmApiKey
                    configStore.llmModel = llmModel
                    saved = true
                },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("保存")
            }

            if (saved) {
                Text(
                    "设置已保存",
                    color = MaterialTheme.colorScheme.primary,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
    }
}
