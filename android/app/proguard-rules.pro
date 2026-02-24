# Keep JNI methods from being removed or renamed by R8/ProGuard.
-keep class com.tingyuxuan.core.NativeCore { *; }

# Keep Kotlin coroutines
-keepclassmembernames class kotlinx.** {
    volatile <fields>;
}
