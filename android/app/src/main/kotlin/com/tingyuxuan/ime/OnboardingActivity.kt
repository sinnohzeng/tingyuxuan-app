package com.tingyuxuan.ime

import android.Manifest
import android.content.ComponentName
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.provider.Settings
import android.view.inputmethod.InputMethodManager
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect

/**
 * 首次使用引导界面 — 引导用户完成 IME 启用、权限授予、API Key 配置。
 *
 * 作为 LAUNCHER Activity，用户安装后首先看到这个界面。
 * 自动检测各步骤完成状态，已完成的步骤显示绿色勾号。
 */
class OnboardingActivity : ComponentActivity() {

    private var audioPermissionGranted = mutableStateOf(false)

    private val requestPermissionLauncher =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            audioPermissionGranted.value = granted
            if (!granted) {
                Toast.makeText(this, "录音权限是语音输入的必要条件", Toast.LENGTH_LONG).show()
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        audioPermissionGranted.value = ContextCompat.checkSelfPermission(
            this, Manifest.permission.RECORD_AUDIO
        ) == PackageManager.PERMISSION_GRANTED

        setContent {
            MaterialTheme {
                OnboardingScreen(
                    audioPermissionGranted = audioPermissionGranted,
                    onEnableIME = { openIMESettings() },
                    onRequestAudioPermission = { requestAudioPermission() },
                    onOpenSettings = { openAppSettings() },
                    isIMEEnabled = { checkIMEEnabled() },
                    isIMESelected = { checkIMESelected() },
                    isConfigured = { ConfigStore(this).isConfigured },
                    onSelectIME = { showInputMethodPicker() },
                    onFinish = { finish() },
                )
            }
        }
    }

    private fun checkIMEEnabled(): Boolean {
        val imm = getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager
        val component = ComponentName(this, TingYuXuanIMEService::class.java)
        return imm.enabledInputMethodList.any {
            it.component == component
        }
    }

    private fun checkIMESelected(): Boolean {
        val currentIME = Settings.Secure.getString(contentResolver, Settings.Secure.DEFAULT_INPUT_METHOD)
        val component = ComponentName(this, TingYuXuanIMEService::class.java)
        return currentIME == component.flattenToString()
    }

    private fun openIMESettings() {
        startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS))
    }

    private fun requestAudioPermission() {
        requestPermissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
    }

    private fun openAppSettings() {
        startActivity(Intent(this, SettingsActivity::class.java))
    }

    private fun showInputMethodPicker() {
        val imm = getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager
        imm.showInputMethodPicker()
    }
}

@Composable
private fun OnboardingScreen(
    audioPermissionGranted: State<Boolean>,
    onEnableIME: () -> Unit,
    onRequestAudioPermission: () -> Unit,
    onOpenSettings: () -> Unit,
    isIMEEnabled: () -> Boolean,
    isIMESelected: () -> Boolean,
    isConfigured: () -> Boolean,
    onSelectIME: () -> Unit,
    onFinish: () -> Unit,
) {
    // 每次 resume 时刷新状态（用户从系统设置返回后）
    var imeEnabled by remember { mutableStateOf(false) }
    var imeSelected by remember { mutableStateOf(false) }
    var apiConfigured by remember { mutableStateOf(false) }

    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
        imeEnabled = isIMEEnabled()
        imeSelected = isIMESelected()
        apiConfigured = isConfigured()
    }

    val allDone = imeEnabled && audioPermissionGranted.value && apiConfigured

    Scaffold { padding ->
        Column(
            modifier = Modifier
                .padding(padding)
                .padding(24.dp)
                .fillMaxSize()
                .verticalScroll(rememberScrollState()),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Spacer(modifier = Modifier.height(32.dp))

            Text(
                text = "听语轩",
                fontSize = 32.sp,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.primary,
            )

            Text(
                text = "AI 智能语音输入",
                fontSize = 16.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            Spacer(modifier = Modifier.height(32.dp))

            // Step 1: 启用输入法
            SetupStepCard(
                stepNumber = 1,
                title = "启用输入法",
                description = "在系统设置中启用「听语轩」键盘",
                isDone = imeEnabled,
                buttonLabel = "去启用",
                onAction = onEnableIME,
            )

            Spacer(modifier = Modifier.height(12.dp))

            // Step 2: 录音权限
            SetupStepCard(
                stepNumber = 2,
                title = "录音权限",
                description = "语音输入需要使用麦克风",
                isDone = audioPermissionGranted.value,
                buttonLabel = "授权",
                onAction = onRequestAudioPermission,
            )

            Spacer(modifier = Modifier.height(12.dp))

            // Step 3: 配置 API Key
            SetupStepCard(
                stepNumber = 3,
                title = "配置 API Key",
                description = "设置语音识别 (STT) 和语言模型 (LLM) 的 API Key",
                isDone = apiConfigured,
                buttonLabel = "去配置",
                onAction = onOpenSettings,
            )

            Spacer(modifier = Modifier.height(12.dp))

            // Step 4: 选择输入法（可选）
            AnimatedVisibility(visible = imeEnabled && !imeSelected) {
                SetupStepCard(
                    stepNumber = 4,
                    title = "切换到听语轩",
                    description = "将听语轩设为当前输入法",
                    isDone = imeSelected,
                    buttonLabel = "切换",
                    onAction = onSelectIME,
                )
            }

            Spacer(modifier = Modifier.height(24.dp))

            // 完成按钮
            AnimatedVisibility(visible = allDone) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(
                        text = "设置完成！",
                        fontSize = 18.sp,
                        fontWeight = FontWeight.Medium,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = "在任意输入框中切换到听语轩键盘，\n点击麦克风按钮开始语音输入。",
                        textAlign = TextAlign.Center,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Button(
                        onClick = onFinish,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text("开始使用")
                    }
                }
            }

            // 未完成时的提示
            AnimatedVisibility(visible = !allDone) {
                Text(
                    text = "请完成以上步骤后开始使用",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 16.dp),
                )
            }
        }
    }
}

@Composable
private fun SetupStepCard(
    stepNumber: Int,
    title: String,
    description: String,
    isDone: Boolean,
    buttonLabel: String,
    onAction: () -> Unit,
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = if (isDone) {
                MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.3f)
            } else {
                MaterialTheme.colorScheme.surfaceVariant
            }
        ),
    ) {
        Row(
            modifier = Modifier
                .padding(16.dp)
                .fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // 步骤编号 / 完成标记
            Surface(
                shape = MaterialTheme.shapes.small,
                color = if (isDone) {
                    MaterialTheme.colorScheme.primary
                } else {
                    MaterialTheme.colorScheme.outline
                },
                modifier = Modifier.size(32.dp),
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Text(
                        text = if (isDone) "\u2713" else "$stepNumber",
                        color = MaterialTheme.colorScheme.onPrimary,
                        fontWeight = FontWeight.Bold,
                    )
                }
            }

            Spacer(modifier = Modifier.width(12.dp))

            // 文字说明
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = title,
                    fontWeight = FontWeight.Medium,
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    text = description,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            // 操作按钮
            if (!isDone) {
                Spacer(modifier = Modifier.width(8.dp))
                FilledTonalButton(onClick = onAction) {
                    Text(buttonLabel)
                }
            }
        }
    }
}
