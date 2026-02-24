# JNI 桥接层

## 概述

`tingyuxuan-jni` crate 提供 Rust ↔ Kotlin JNI 桥接，使 Android IME 能调用 `tingyuxuan-core` 引擎。

## 架构

```
Kotlin (NativeCore.kt)
  ↓ JNI call (jlong handle)
Rust (tingyuxuan-jni)
  ↓ handle table lookup
Arc<Pipeline> (tingyuxuan-core)
  ↓ STT → LLM pipeline
JSON result string
  ↓ JNI return
Kotlin (parse JSON)
```

## JNI 导出函数

| Java 方法 | Rust 函数 | 签名 |
|-----------|-----------|------|
| `NativeCore.initPipeline(configJson)` | `Java_com_tingyuxuan_core_NativeCore_initPipeline` | `(Ljava/lang/String;)J` |
| `NativeCore.processAudio(handle, audioPath, mode, selectedText)` | `Java_com_tingyuxuan_core_NativeCore_processAudio` | `(JLjava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;` |
| `NativeCore.destroyPipeline(handle)` | `Java_com_tingyuxuan_core_NativeCore_destroyPipeline` | `(J)V` |

## Handle Table

详见 [ADR-0007](../architecture/adr/0007-android-native-ime.md)。

- **注册**：`register_handle(Arc<Pipeline>) → u64`
- **查找**：`get_handle(u64) → Result<Arc<Pipeline>, String>`
- **销毁**：`remove_handle(u64) → bool`

Handle ID 从 1 开始单调递增，0 表示无效。

## 构建

```bash
# 安装工具
cargo install cargo-ndk
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

# 设置 Android NDK
export ANDROID_NDK_HOME=/path/to/ndk

# 构建
cd crates/tingyuxuan-jni
./build-android.sh
```

输出的 `.so` 文件需要复制到 `android/app/src/main/jniLibs/<abi>/`。

## 测试

```bash
cargo test -p tingyuxuan-jni
```
