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
# Compose runtime 需要的反射
-keep class androidx.compose.** { *; }
-dontwarn androidx.compose.**

# --- EncryptedSharedPreferences ---
-keep class androidx.security.crypto.** { *; }

# --- JSON 解析 ---
-keep class org.json.** { *; }

# --- 保留异常信息用于调试 ---
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile
