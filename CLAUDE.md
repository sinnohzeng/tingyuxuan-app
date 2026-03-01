# CLAUDE.md — AI 开发助手上下文

> 本文件为 AI 编码助手提供项目上下文，帮助快速理解项目结构、避免已知陷阱。

## 项目概览

听语轩（TingYuXuan）— AI 驱动的智能语音输入工具。核心管线：语音录制 → WAV 编码 → 多模态 LLM 一步识别+润色 → 系统级文本注入。

## 技术栈

| 层 | 技术 |
|----|------|
| Desktop | Tauri 2.10 + React 19 + Zustand 5 + Tailwind CSS 4 (Linux, macOS, Windows) |
| Android | Kotlin (AGP 9.0.1 内置) + Compose (BOM 2026.02.00) + InputMethodService |
| Backend | Rust 2024 edition + tokio 1.x + reqwest 0.13 + rusqlite 0.38 |
| Audio | cpal 0.17 + hound 3.5 (optional feature, 桌面专用) |
| Testing | 99 Rust + 42 vitest + 13 JNI + 7 Android 单元测试 |

## 项目结构

```
crates/tingyuxuan-core/   Rust 核心库（平台无关）
crates/tingyuxuan-jni/    Android JNI 桥接
src-tauri/                Tauri 桌面应用
src/                      React 前端
android/                  Android 原生输入法
docs/                     DDD 文档体系
.github/workflows/        CI (ci.yml) + Release (release.yml)
```

## 常用命令

```bash
# 测试
cargo test -p tingyuxuan-core          # 116 Rust tests
npm test                                # 44 frontend tests
npx tsc --noEmit                        # TypeScript 类型检查

# 本机无法完整编译 Tauri（缺 webkit2gtk 头文件），但可以：
cargo check -p tingyuxuan-core --no-default-features   # 不含音频的核心检查
cargo test -p tingyuxuan-jni                            # JNI 测试 (7 tests)
```

## CI/Release 构建关键经验

> 详见 `docs/guides/ci-release-notes.md`

### Tauri 2.x target 目录差异

| 命令 | target 目录 |
|------|------------|
| `npx tauri build` | `target/release/bundle/` (workspace root) |
| `cargo build --manifest-path src-tauri/Cargo.toml` | `src-tauri/target/` |

Release workflow 用 `npx tauri build`，CI workflow 用 `cargo build --manifest-path`，路径不同！

### AGP 9.0.1 Breaking Changes 清单

AGP 9.0 是大版本更新，以下全部在 v0.4.0 构建中踩过：

1. **Gradle 版本**：最低 9.1.0（不是 8.x）
2. **Kotlin 插件内置**：必须从 build.gradle.kts 删除 `org.jetbrains.kotlin.android`
3. **kotlinOptions 移除**：改用 `kotlin { compilerOptions { jvmTarget = JvmTarget.JVM_21 } }`
4. **pluginManagement 必需**：settings.gradle.kts 需要 `google()` + `mavenCentral()` 仓库
5. **内存需求**：R8 + AGP 9.0 在 GitHub Actions 上需要 `-Xmx4g` 堆（gradle.properties）
6. **XML 主题**：Compose Material3 不提供 XML 主题资源，AndroidManifest 用系统主题

### GitHub Actions Release 工作流

- **架构**：fan-out/fan-in — 4 个 build job (Linux, Windows, macOS, Android) 各自 `upload-artifact`，1 个 `create-release` job 统一 `download-artifact` + 创建 Release
- **Tag 触发**：tags 必须和 commits **分开推送**，否则可能不触发 `on: push: tags`
- **Artifact 路径**：`upload-artifact@v4` 保留相对目录结构；`download-artifact@v4` 在 `<artifact-name>/` 子目录下展开
- **Android 构建链**：`cargo-ndk` 编译 .so → 复制到 `jniLibs/` → `gradlew assembleRelease`

### 跨平台 #[cfg] 代码验证（重要）

`#[cfg(target_os = "...")]` 门控的代码**只能被对应平台 CI 验证**。本地 Linux clippy 完全跳过 macOS/Windows 代码。常见陷阱包括 core-graphics C 绑定缺少 PartialEq、!Send 原始指针类型、API 风格差异等。详见 `docs/guides/ci-release-notes.md` #13、#19–#23。

## 代码质量红线（硬阈值）

| 指标 | 阈值 |
|------|------|
| 单文件行数 | ≤ 800 行 |
| 单函数行数 | ≤ 30 行（含 JSX return） |
| 嵌套层级 | ≤ 3 层 |
| 分支数量 | ≤ 3 个/函数 |

超过阈值必须拆分，无例外。

## 开发约定

- **语言**：UI 和文档用中文，commit message 用中文，代码注释中文
- **文档驱动 (DDD)**：文档是功能规格，代码实现文档描述
- **唯一真值 (SSOT)**：每类信息只有一个权威来源，跨文档引用不重复
- **快捷键默认值**：Linux/Windows: RAlt（听写）、Shift+RAlt（翻译）、Alt+Space（AI 助手）、Esc（取消）；macOS: Fn（听写）、⌥T（翻译）、⌃Space（AI 助手）、Esc（取消）
- **Mock 音频**：`TINGYUXUAN_MOCK_AUDIO=1` 环境变量启用录音 mock 模式
- **许可证**：Source-Available（代码公开仅供参考和学习），详见 LICENSE
