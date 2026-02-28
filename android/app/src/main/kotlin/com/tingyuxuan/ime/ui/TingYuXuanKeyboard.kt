package com.tingyuxuan.ime.ui

import android.content.Context
import android.view.View
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.LifecycleOwner
import com.tingyuxuan.ime.model.*
import kotlinx.coroutines.flow.StateFlow

/**
 * Compose 键盘 UI — 状态驱动的纯函数式渲染。
 *
 * UI = f(IMEState)
 * 所有用户操作通过 [KeyboardIntent] 传递给 Service 处理。
 */
object TingYuXuanKeyboard {

    fun createView(
        context: Context,
        state: StateFlow<IMEState>,
        lifecycleOwner: LifecycleOwner,
        onIntent: (KeyboardIntent) -> Unit,
    ): View {
        return ComposeView(context).apply {
            setContent {
                MaterialTheme {
                    val currentState by state.collectAsState()
                    KeyboardContent(
                        state = currentState,
                        onIntent = onIntent,
                    )
                }
            }
        }
    }
}

@Composable
private fun KeyboardContent(
    state: IMEState,
    onIntent: (KeyboardIntent) -> Unit,
) {
    // 从 IMEState 派生当前模式，保持 UI 和状态机同步
    var selectedMode by remember { mutableStateOf(ProcessingMode.Dictate) }
    LaunchedEffect(state) {
        val derivedMode = when (state) {
            is IMEState.Idle -> state.currentMode
            is IMEState.Recording -> state.mode
            is IMEState.Processing -> state.mode
            is IMEState.Done -> state.mode
            is IMEState.Error -> state.failedMode
        }
        selectedMode = derivedMode
    }

    Surface(
        modifier = Modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 4.dp,
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            when (state) {
                is IMEState.Idle -> IdleContent(
                    selectedMode = selectedMode,
                    isPasswordField = state.isPasswordField,
                    onModeSelected = { mode ->
                        selectedMode = mode
                        onIntent(KeyboardIntent.SwitchMode(mode))
                    },
                    onStartRecording = {
                        onIntent(KeyboardIntent.StartRecording(selectedMode))
                    },
                    onOpenSettings = { onIntent(KeyboardIntent.OpenSettings) },
                )

                is IMEState.Recording -> RecordingContent(
                    mode = state.mode,
                    amplitude = state.amplitude,
                    startTimeMs = state.startTimeMs,
                    onStop = { onIntent(KeyboardIntent.StopRecording) },
                    onCancel = { onIntent(KeyboardIntent.CancelRecording) },
                )

                is IMEState.Processing -> ProcessingContent(
                    mode = state.mode,
                    stage = state.stage,
                )

                is IMEState.Done -> DoneContent(
                    text = state.text,
                    mode = state.mode,
                )

                is IMEState.Error -> ErrorContent(
                    code = state.code,
                    message = state.message,
                    onAction = {
                        when (state.code.userAction) {
                            UserAction.Retry -> onIntent(KeyboardIntent.Retry)
                            UserAction.OpenSettings -> onIntent(KeyboardIntent.OpenSettings)
                            UserAction.CheckApiKey -> onIntent(KeyboardIntent.OpenSettings)
                            UserAction.RequestPermission -> onIntent(KeyboardIntent.OpenSettings)
                            UserAction.Reinstall -> onIntent(KeyboardIntent.ClearError)
                            UserAction.Dismiss -> onIntent(KeyboardIntent.ClearError)
                        }
                    },
                    onDismiss = { onIntent(KeyboardIntent.ClearError) },
                )
            }
        }
    }
}

// --- Idle State ---

@Composable
private fun IdleContent(
    selectedMode: ProcessingMode,
    isPasswordField: Boolean,
    onModeSelected: (ProcessingMode) -> Unit,
    onStartRecording: () -> Unit,
    onOpenSettings: () -> Unit,
) {
    // 模式选择栏
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceEvenly,
    ) {
        ProcessingMode.entries.filter { it != ProcessingMode.Edit }.forEach { mode ->
            ModeChip(
                label = mode.label,
                selected = mode == selectedMode,
                enabled = !isPasswordField,
                onSelect = { onModeSelected(mode) },
            )
        }
        // 设置按钮
        FilledTonalButton(
            onClick = onOpenSettings,
            contentPadding = PaddingValues(horizontal = 12.dp, vertical = 0.dp),
            modifier = Modifier.height(32.dp),
        ) {
            Text("\u2699", fontSize = 16.sp)
        }
    }

    Spacer(modifier = Modifier.height(8.dp))

    // 麦克风按钮
    Button(
        onClick = onStartRecording,
        enabled = !isPasswordField,
        colors = ButtonDefaults.buttonColors(
            containerColor = MaterialTheme.colorScheme.primary,
        ),
        modifier = Modifier
            .fillMaxWidth()
            .height(48.dp),
    ) {
        Text(
            text = if (isPasswordField) "密码框不可语音输入" else "开始录音 \uD83C\uDFA4",
            fontSize = 16.sp,
        )
    }
}

// --- Recording State ---

@Composable
private fun RecordingContent(
    mode: ProcessingMode,
    amplitude: Float,
    startTimeMs: Long,
    onStop: () -> Unit,
    onCancel: () -> Unit,
) {
    // 录音时长
    var elapsedSeconds by remember { mutableIntStateOf(0) }
    LaunchedEffect(startTimeMs) {
        while (true) {
            elapsedSeconds = ((System.currentTimeMillis() - startTimeMs) / 1000).toInt()
            kotlinx.coroutines.delay(200)
        }
    }

    Text(
        text = "${mode.label}模式 · ${elapsedSeconds}s",
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )

    Spacer(modifier = Modifier.height(4.dp))

    // 振幅指示条
    AmplitudeBar(amplitude = amplitude)

    Spacer(modifier = Modifier.height(8.dp))

    // 停止 / 取消按钮
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        OutlinedButton(
            onClick = onCancel,
            modifier = Modifier.weight(1f).height(48.dp),
        ) {
            Text("取消")
        }
        Button(
            onClick = onStop,
            colors = ButtonDefaults.buttonColors(
                containerColor = MaterialTheme.colorScheme.error,
            ),
            modifier = Modifier.weight(2f).height(48.dp),
        ) {
            Text("停止录音", fontSize = 16.sp)
        }
    }
}

@Composable
private fun AmplitudeBar(amplitude: Float) {
    val animatedAmplitude by animateFloatAsState(
        targetValue = amplitude,
        animationSpec = tween(durationMillis = 100),
        label = "amplitude",
    )

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(6.dp)
            .clip(RoundedCornerShape(3.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant),
    ) {
        Box(
            modifier = Modifier
                .fillMaxHeight()
                // 0.02 minimum prevents visual disappearance during brief silence
                .fillMaxWidth(fraction = animatedAmplitude.coerceIn(0.02f, 1f))
                .clip(RoundedCornerShape(3.dp))
                .background(MaterialTheme.colorScheme.primary),
        )
    }
}

// --- Processing State ---

@Composable
private fun ProcessingContent(
    mode: ProcessingMode,
    stage: ProcessingStage,
) {
    // 加载动画
    val infiniteTransition = rememberInfiniteTransition(label = "processing")
    val dotCount by infiniteTransition.animateFloat(
        initialValue = 0f,
        targetValue = 3f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 900),
            repeatMode = RepeatMode.Restart,
        ),
        label = "dots",
    )
    val dots = ".".repeat(dotCount.toInt() + 1)

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .height(80.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        LinearProgressIndicator(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 32.dp),
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "${stage.label}$dots",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

// --- Done State ---

@Composable
private fun DoneContent(
    text: String,
    mode: ProcessingMode,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .height(80.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = "\u2713 已输入",
            fontWeight = FontWeight.Medium,
            color = MaterialTheme.colorScheme.primary,
        )
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = text,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(horizontal = 16.dp),
        )
    }
}

// --- Error State ---

@Composable
private fun ErrorContent(
    code: ErrorCode,
    message: String,
    onAction: () -> Unit,
    onDismiss: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = message,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(horizontal = 8.dp),
        )

        Spacer(modifier = Modifier.height(8.dp))

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
        ) {
            OutlinedButton(onClick = onDismiss) {
                Text("关闭")
            }
            Button(onClick = onAction) {
                Text(code.userAction.buttonLabel)
            }
        }
    }
}

// --- Shared Components ---

@Composable
private fun ModeChip(
    label: String,
    selected: Boolean,
    enabled: Boolean,
    onSelect: () -> Unit,
) {
    FilterChip(
        selected = selected,
        onClick = onSelect,
        enabled = enabled,
        label = { Text(label) },
    )
}
