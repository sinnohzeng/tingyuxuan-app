# ADR-0007: Android 原生 IME 架构

**状态**: Accepted
**日期**: 2026-02 (Phase 5)

## 背景

听语轩需要支持 Android 平台（PRD P2 优先级）。Android 的文本输入采用 InputMethodService 机制，
与桌面端的全局快捷键 + 文本注入完全不同。需要选择合适的 Native 集成方式。

## 决策

采用 **Kotlin IME + Rust JNI 桥接** 架构。

### 核心组件

1. **`tingyuxuan-jni` crate**（Rust）：cdylib 输出 `libtingyuxuan_jni.so`
   - 通过 JNI 导出 `initPipeline`、`processAudio`、`destroyPipeline`
   - 使用 generation-based handle table（`Mutex<HashMap<u64, Arc<Pipeline>>>`）管理 Pipeline 生命周期
   - 不传递裸指针，避免 use-after-free / double-free

2. **`tingyuxuan-core`**（Rust）：`default-features = false`（禁用 cpal/hound）
   - 复用多模态 LLM、Pipeline、配置与错误模型逻辑
   - Audio 录音由 Android `AudioRecord` API 完成（Kotlin 侧）

3. **Android IME**（Kotlin + Compose）：
   - `InputMethodService` 子类处理输入法生命周期
   - `AudioRecord` 录制 16kHz mono WAV
   - `NativeCore.kt` JNI 接口调用 Rust
   - `currentInputConnection.commitText()` 注入文本（无需剪贴板）

### Handle Table 安全设计

```
Kotlin (jlong handle) → Rust HashMap<u64, Arc<Pipeline>> → Pipeline
```

- ID 从 1 开始单调递增，不复用（防止 ABA 问题）
- 每次 JNI 调用前通过 `get_handle()` 验证 handle 有效性
- `destroyPipeline` 从 table 移除 handle，Arc 引用计数归零后自动释放

## 后果

**正面**：
- 最大化代码复用：多模态处理与 Pipeline 逻辑零重复
- Handle table 消除了所有 JNI unsafe 内存管理风险
- Android 原生 IME 体验优于 WebView 方案
- `commitText()` 直接输入，无需剪贴板 hack

**负面**：
- 需要维护 Rust 交叉编译工具链（cargo-ndk + Android NDK）
- JNI 层增加了调用开销（跨语言边界）
- `.so` 文件增加 APK 体积（~5-10MB per arch）
- 需要同时掌握 Kotlin 和 Rust 两套技术栈

## 备选方案

| 方案 | 未选择原因 |
|------|-----------|
| WebView IME | Android WebView 无法作为 InputMethodService |
| 纯 Kotlin 重写 | 大量重复代码，多模态处理与错误模型需要双份维护 |
| Box::into_raw 指针传递 | use-after-free / double-free 风险高 |
| UniFFI 自动绑定 | 增加构建复杂度，handle table 足够简单 |
