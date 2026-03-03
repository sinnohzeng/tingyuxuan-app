# JNI 桥接层

## 概述

`tingyuxuan-jni` crate 提供 Android Kotlin ↔ Rust 的 JNI 接口，桥接到 `tingyuxuan-core` 的单步多模态 Pipeline。

**源文件:** `crates/tingyuxuan-jni/src/lib.rs`

---

## 架构

```
Kotlin IME
  ↓ JNI (jlong handle)
tingyuxuan-jni
  ↓ handle table lookup
Arc<Pipeline>
  ↓ startStreaming / sendAudioChunk / stopStreaming
JSON result string
  ↓
Kotlin 解析并更新 UI
```

关键点：
- JNI 边界不传裸指针，使用 handle table 管理 `Arc<Pipeline>`。
- 录音为分段 PCM 传输：先建会话，再持续送帧，最后停止并处理。
- 所有结果统一返回 JSON，Kotlin 侧按 `success/error_code/message` 解析。

---

## JNI 导出方法

| Java 方法 | Rust 函数 | 签名 |
|-----------|-----------|------|
| `NativeCore.initPipeline(configJson)` | `Java_com_tingyuxuan_core_NativeCore_initPipeline` | `(Ljava/lang/String;)J` |
| `NativeCore.destroyPipeline(handle)` | `Java_com_tingyuxuan_core_NativeCore_destroyPipeline` | `(J)V` |
| `NativeCore.startStreaming(handle, mode, contextJson)` | `Java_com_tingyuxuan_core_NativeCore_startStreaming` | `(JLjava/lang/String;Ljava/lang/String;)Ljava/lang/String;` |
| `NativeCore.sendAudioChunk(handle, pcmData)` | `Java_com_tingyuxuan_core_NativeCore_sendAudioChunk` | `(J[S)Z` |
| `NativeCore.stopStreaming(handle)` | `Java_com_tingyuxuan_core_NativeCore_stopStreaming` | `(J)Ljava/lang/String;` |
| `NativeCore.cancelProcessing(handle)` | `Java_com_tingyuxuan_core_NativeCore_cancelProcessing` | `(J)V` |
| `NativeCore.validateConfig(configJson)` | `Java_com_tingyuxuan_core_NativeCore_validateConfig` | `(Ljava/lang/String;)Ljava/lang/String;` |
| `NativeCore.testConnection(configJson, service)` | `Java_com_tingyuxuan_core_NativeCore_testConnection` | `(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;` |
| `NativeCore.getVersion()` | `Java_com_tingyuxuan_core_NativeCore_getVersion` | `()Ljava/lang/String;` |

兼容约束：JNI 方法名和签名保持稳定，Android 调用层可无破坏升级。

---

## Handle Table 与会话表

### Pipeline Handle Table

- `register_handle(Arc<Pipeline>) -> u64`
- `get_handle(u64) -> Result<Arc<Pipeline>, String>`
- `remove_handle(u64) -> Result<bool, String>`

规则：
- handle 从 1 开始递增，0 为无效值。
- 销毁 handle 后由 Arc 引用计数自动释放资源。

### Recording Session Table

JNI 层另有录音会话表（`handle_id -> RecordingSession`）：
- `startStreaming` 创建 `AudioBuffer + ProcessingRequest + CancellationToken`
- `sendAudioChunk` 追加 PCM 样本
- `stopStreaming` `take` 会话并调用 `pipeline.process_audio()`

---

## 运行时与线程

- 使用 `OnceLock<tokio::Runtime>` 复用全局 Runtime，避免频繁构建。
- `stopStreaming` / `testConnection` 在 Runtime 中阻塞等待异步结果。
- 取消操作通过 `CancellationToken` 传递到 Pipeline。

---

## 错误协议

返回 JSON 统一结构：

```json
{
  "success": false,
  "error_code": "invalid_handle",
  "message": "No active recording session for handle: 123",
  "user_action": "dismiss"
}
```

- 成功返回：`{"success": true, ...}`
- 失败返回：`success=false` + `error_code/message/user_action`
- Rust `StructuredError` 会映射为上述格式，便于 Kotlin 侧统一处理

---

## 构建与测试

```bash
cargo test -p tingyuxuan-jni
```

Android 交叉编译仍使用 `cargo-ndk` 输出 `.so` 到 `jniLibs/<abi>/`。
