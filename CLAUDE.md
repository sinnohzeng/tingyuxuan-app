# CLAUDE.md — AI 开发助手上下文

> 本文件为 AI 编码助手提供项目上下文，帮助快速理解项目结构、避免已知陷阱。

## 项目概览

听语轩（TingYuXuan）— AI 驱动的智能语音输入工具。核心管线：语音录制 → MP3（失败回退 WAV）编码 → 多模态 LLM 一步识别+润色 → 系统级文本注入。前端采用 feature-based 目录结构 + Fluent UI 2 组件库。

当前执行计划（SSOT）：`docs/plan/phase-7-voice-mvp-remediation.md`

## 技术栈

| 层 | 技术 |
|----|------|
| Desktop | Tauri 2.10 + React 19 + Zustand 5 + Tailwind CSS 4 (Linux, macOS, Windows) |
| Android | Kotlin (AGP 9.0.1 内置) + Compose (BOM 2026.02.00) + InputMethodService |
| Backend | Rust 2024 edition + tokio 1.x + reqwest 0.13 + rusqlite 0.38 |
| Audio | cpal 0.17 + hound 3.5 (optional feature, 桌面专用) |
| UI 组件库 | Fluent UI 2 (@fluentui/react-components) |
| 可观测性 | Sentry 0.42 + tauri-plugin-sentry 0.5 (崩溃/错误) + SLS Web Tracking (埋点) |
| Testing | 131 Rust + 71 vitest + 13 JNI + 7 Android 单元测试 |

## 项目结构

```
crates/tingyuxuan-core/   Rust 核心库（平台无关）
crates/tingyuxuan-jni/    Android JNI 桥接
src-tauri/                Tauri 桌面应用
src/                      React 前端（feature-based 目录）
  features/               按功能模块划分
    dashboard/            首页 + 统计卡片
    dictionary/           词典管理
    history/              历史记录
    onboarding/           引导流程
    recording/            录音浮动条 + 结果面板
    settings/             设置对话框
  shared/                 跨功能共享
    components/           MainLayout、ToastHost
    hooks/                useTauriEvent
    lib/                  types、logger、theme、telemetry
    stores/               appStore、uiStore、statsStore
android/                  Android 原生输入法
docs/                     DDD 文档体系
.github/workflows/        CI (ci.yml) + Release (release.yml)
```

## 常用命令

```bash
# 测试
cargo test -p tingyuxuan-core          # 124 Rust tests
npm test                                # 71 frontend tests
npx tsc --noEmit                        # TypeScript 类型检查

# 文档质量
npm run lint:docs                       # Markdown 格式检查（markdownlint-cli2）
npm run lint:links                      # 文档链接校验（需安装 lychee）
npm run changelog:next                  # 预览未发布变更日志（需安装 git-cliff）

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
- **DDD/SSOT 执行闸门（必遵守）**：
  1. 每次功能改动必须同步更新对应唯一模块文档（`docs/modules/*`）
  2. 每次里程碑推进必须同步更新当前执行计划（`docs/plan/phase-7-voice-mvp-remediation.md`）
  3. `CLAUDE.md` 只记录稳定约束、执行入口与常用命令，不写易变实现细节
- **胶水编程原则（Glue Code First）**：优先使用成熟开源库，不重复造轮子；缺什么依赖就装什么；数据收集（埋点、崩溃分析）用专业 SaaS 工具；自己的代码只负责编排和连接
- **错误处理**：所有 hook/store catch 块用 `createLogger` 记录技术细节 + `uiStore.showToast()` 通知用户，禁止静默吞错或裸 `console.error`
- **快捷键默认值**：Linux/Windows: RAlt（听写）、Shift+RAlt（翻译）、Alt+Space（AI 助手）、Esc（取消）；macOS: Fn（听写）、⌥T（翻译）、⌃Space（AI 助手）、Esc（取消）
- **Mock 音频**：`TINGYUXUAN_MOCK_AUDIO=1` 环境变量启用录音 mock 模式
- **localStorage 例外**：项目唯一使用 localStorage 的地方是 `onboarding_complete` 标记（引导状态是纯前端关注点，需同步判断避免首帧闪烁）
- **计划文档目录**：所有开发计划统一存放在 `docs/plan/`，禁止使用 `docs/plans/`
- **许可证**：Source-Available（代码公开仅供参考和学习），详见 LICENSE

## Tauri 命令清单

| 命令 | 参数 | 返回值 | 用途 |
|------|------|--------|------|
| `start_recording` | `mode: String` | `String` | 开始录音，返回 session_id |
| `stop_recording` | — | `String` | 停止录音，返回处理状态 |
| `cancel_recording` | — | `()` | 取消录音 |
| `get_config` | — | `AppConfig` | 获取配置 |
| `save_config` | `config: AppConfig` | `()` | 保存配置 |
| `get_api_key` | `service: String` | `Option<String>` | 获取 API Key |
| `save_api_key` | `service, key` | `()` | 保存 API Key |
| `test_multimodal_connection` | — | `bool` | 测试多模态音频连接 |
| `inject_text` | `text: String` | `()` | 注入文本到当前窗口 |
| `get_recent_history` | `limit: u32` | `Vec<TranscriptRecord>` | 最近历史 |
| `get_history_page` | `limit, offset` | `Vec<TranscriptRecord>` | 分页历史 |
| `search_history` | `query, limit` | `Vec<TranscriptRecord>` | 搜索历史 |
| `delete_history` | `id: String` | `()` | 删除记录 |
| `delete_history_batch` | `ids: Vec<String>` | `u64` | 批量删除记录 |
| `clear_history` | — | `u64` | 清空历史，返回删除数 |
| `get_dashboard_stats` | — | `AggregateStats` | 统计数据 |
| `get_dictionary` | — | `Vec<String>` | 获取词典 |
| `add_dictionary_word` | `word: String` | `()` | 添加词汇 |
| `remove_dictionary_word` | `word: String` | `()` | 删除词汇 |
| `check_platform_permissions` | — | `String` | 检查权限状态 |
| `open_permission_settings` | `target: Option<String>` | `()` | 打开权限设置 |
| `report_telemetry_event` | `event: String` | `()` | 前端埋点上报 |
| `is_first_launch` | — | `bool` | 首次启动检查 |
| `list_input_devices` | — | `Vec<AudioDeviceInfo>` | 枚举音频输入设备 |
| `set_input_device` | `device_id: Option<String>` | `()` | 设置输入设备 |

## 环境变量

| 变量 | 用途 | 必需 |
|------|------|------|
| `TINGYUXUAN_MOCK_AUDIO` | `1` 启用录音 mock 模式 | 开发可选 |
| `SENTRY_DSN` | Sentry 错误上报 DSN | 生产必需 |
| `SLS_ENDPOINT` | 阿里云 SLS 区域 endpoint | 埋点必需 |
| `SLS_PROJECT` | SLS Project 名称 | 埋点必需 |
| `SLS_LOGSTORE` | SLS Logstore 名称 | 埋点必需 |

## MCP 服务器（AI 工具链）

项目根目录 `.mcp.json` 定义了 Claude Code 可调用的 MCP 服务器。API Key 等敏感信息通过系统环境变量注入（`.mcp.json` 不会读取 `.env` 文件）。

### MiniMax MCP（图像/音频/视频生成）

**提供工具**：`text_to_image`、`text_to_audio`、`generate_video`、`music_generation` 等

**配置步骤**：

**第 1 步：获取 API Key**

前往 [MiniMax 开放平台](https://platform.minimaxi.com/user-center/basic-information/interface-key) 创建 API Key。

**第 2 步：设置系统环境变量**

| 变量 | 用途 | 示例 |
|------|------|------|
| `MINIMAX_API_KEY` | **必需** — MiniMax API 密钥 | `sk-...` |
| `MINIMAX_MCP_BASE_PATH` | 可选 — 生成文件输出目录（默认 `./`） | 项目根目录绝对路径 |

**Windows（PowerShell 管理员，永久生效，需重启终端）**：
```powershell
[Environment]::SetEnvironmentVariable("MINIMAX_API_KEY", "你的API密钥", "User")
[Environment]::SetEnvironmentVariable("MINIMAX_MCP_BASE_PATH", "C:\Users\你的用户名\workspace\tingyuxuan-app", "User")
```

**macOS（添加到 `~/.zshrc`）**：
```bash
export MINIMAX_API_KEY="你的API密钥"
export MINIMAX_MCP_BASE_PATH="$HOME/workspace/tingyuxuan-app"
```

**Linux（添加到 `~/.bashrc` 或 `~/.zshrc`）**：
```bash
export MINIMAX_API_KEY="你的API密钥"
export MINIMAX_MCP_BASE_PATH="$HOME/workspace/tingyuxuan-app"
```

**第 3 步：Windows 专用 — 添加本地 MCP 覆盖**

`.mcp.json` 中 `npx` 命令在 macOS/Linux 上直接可用，但 Windows 上 `npx` 是 `.cmd` 文件，需要 `cmd /c` 包装。Windows 用户需额外执行一次本地覆盖（写入 `~/.claude.json`，不影响 Git）：

```bash
claude mcp add minimax --transport stdio -s user -e MINIMAX_API_KEY=%MINIMAX_API_KEY% -e MINIMAX_MCP_BASE_PATH=%MINIMAX_MCP_BASE_PATH% -e MINIMAX_API_HOST=https://api.minimaxi.com -e MINIMAX_RESOURCE_MODE=local -- cmd /c npx -y minimax-mcp-js
```

macOS/Linux 用户无需此步骤，项目 `.mcp.json` 直接生效。

**第 4 步：验证**

重启 Claude Code，输入 `/mcp` 确认 minimax 服务器状态为 connected。

### Sentry MCP（错误/崩溃查询）

**提供工具**：`sentry_list_issues`、`sentry_get_issue`、`sentry_get_latest_event`、`sentry_search`

**环境变量**：`SENTRY_URL`、`SENTRY_AUTH_TOKEN`、`SENTRY_ORG`、`SENTRY_PROJECT`

### SLS MCP（埋点数据查询）

**提供工具**：`sls_query`、`sls_get_session`、`sls_error_stats`、`sls_performance`

**环境变量**：`SLS_ENDPOINT`、`SLS_PROJECT`、`SLS_LOGSTORE`、`SLS_ACCESS_KEY_ID`、`SLS_ACCESS_KEY_SECRET`
