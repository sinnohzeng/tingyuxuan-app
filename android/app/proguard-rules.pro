# ============================================================================
# TingYuXuan (听语轩) ProGuard/R8 Rules
# ============================================================================

# --- JNI 接口 ---
# NativeCore 的类名和方法名被 JNI 引用，不可混淆或移除
-keep class com.tingyuxuan.core.NativeCore { *; }

# --- IME Service ---
# InputMethodService 通过 AndroidManifest 引用，需要保留
-keep class com.tingyuxuan.ime.TingYuXuanIMEService { *; }

# --- Kotlin Coroutines ---
-keepclassmembernames class kotlinx.** {
    volatile <fields>;
}

# --- Compose ---
# Compose runtime 和 platform 反射（精确规则，避免保留全部 Compose 类）
-keep class androidx.compose.runtime.** { *; }
-keep class androidx.compose.ui.platform.** { *; }
-keepclassmembers class * {
    @androidx.compose.runtime.Composable <methods>;
}
-dontwarn androidx.compose.**

# --- EncryptedSharedPreferences ---
-keep class androidx.security.crypto.** { *; }

# --- JSON 解析 ---
-keep class org.json.** { *; }

# --- 保留异常信息用于调试 ---
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile
