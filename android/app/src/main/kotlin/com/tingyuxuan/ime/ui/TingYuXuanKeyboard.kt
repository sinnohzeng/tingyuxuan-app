package com.tingyuxuan.ime.ui

import android.content.Context
import android.view.View
import android.widget.FrameLayout
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.unit.dp
import com.tingyuxuan.ime.KeyboardAction

/**
 * Compose-based keyboard UI for TingYuXuan IME.
 *
 * Displays a recording button, mode selector, and volume visualization.
 */
object TingYuXuanKeyboard {

    fun createView(
        context: Context,
        onAction: (KeyboardAction) -> Unit,
    ): View {
        return ComposeView(context).apply {
            setContent {
                MaterialTheme {
                    KeyboardContent(onAction = onAction)
                }
            }
        }
    }
}

@Composable
private fun KeyboardContent(
    onAction: (KeyboardAction) -> Unit,
) {
    var isRecording by remember { mutableStateOf(false) }
    var currentMode by remember { mutableStateOf("dictate") }

    Surface(
        modifier = Modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 4.dp,
    ) {
        Column(
            modifier = Modifier.padding(8.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            // Mode selector
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceEvenly,
            ) {
                ModeChip("听写", "dictate", currentMode) {
                    currentMode = it
                    onAction(KeyboardAction.SwitchMode(it))
                }
                ModeChip("翻译", "translate", currentMode) {
                    currentMode = it
                    onAction(KeyboardAction.SwitchMode(it))
                }
                ModeChip("AI", "ai_assistant", currentMode) {
                    currentMode = it
                    onAction(KeyboardAction.SwitchMode(it))
                }
            }

            Spacer(modifier = Modifier.height(8.dp))

            // Record button
            Button(
                onClick = {
                    if (isRecording) {
                        isRecording = false
                        onAction(KeyboardAction.StopRecording)
                    } else {
                        isRecording = true
                        onAction(KeyboardAction.StartRecording(currentMode))
                    }
                },
                colors = ButtonDefaults.buttonColors(
                    containerColor = if (isRecording) {
                        MaterialTheme.colorScheme.error
                    } else {
                        MaterialTheme.colorScheme.primary
                    }
                ),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(48.dp),
            ) {
                Text(if (isRecording) "停止录音" else "开始录音")
            }
        }
    }
}

@Composable
private fun ModeChip(
    label: String,
    mode: String,
    currentMode: String,
    onSelect: (String) -> Unit,
) {
    FilterChip(
        selected = mode == currentMode,
        onClick = { onSelect(mode) },
        label = { Text(label) },
    )
}
