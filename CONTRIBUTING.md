# 贡献指南

感谢你对听语轩（TingYuXuan）的关注！本指南帮助你快速搭建开发环境并了解项目的协作约定。

## 项目概览

听语轩是一款 AI 驱动的智能语音输入工具，核心管线：语音录制 → 编码 → 多模态 LLM 一步识别+润色 → 系统级文本注入。

- [产品需求文档（PRD）](docs/prd.md) — 产品定位与功能需求
- [系统架构总览](docs/architecture/overview.md) — 分层架构与技术栈

## 开发环境搭建

### 前置条件

| 工具 | 版本要求 |
|------|----------|
| Rust | stable（2024 edition） |
| Node.js | 22+ |
| npm | 10+ |

### 系统依赖

**Ubuntu / Debian**：

```bash
sudo apt install libasound2-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
sudo apt install xdotool xclip  # X11 文本注入
```

**Windows**：安装 Visual Studio Build Tools（MSVC v14.50+ / Windows SDK 10.0.26100+）。

**macOS**：安装 Xcode Command Line Tools。

### 构建与运行

```bash
git clone https://github.com/sinnohzeng/tingyuxuan-app.git
cd tingyuxuan-app
npm install
npm run dev          # 启动前端开发服务器
npx tauri dev        # 启动 Tauri 开发环境（需系统依赖）
```

## 项目结构

```
crates/tingyuxuan-core/   Rust 核心库（平台无关）
crates/tingyuxuan-jni/    Android JNI 桥接
src-tauri/                Tauri 桌面应用
src/                      React 前端（feature-based 目录）
android/                  Android 原生输入法
docs/                     DDD 文档体系
```

详细结构说明见 [CLAUDE.md](CLAUDE.md)。

## 开发工作流

### 分支命名

- `feat/描述` — 新功能
- `fix/描述` — Bug 修复
- `docs/描述` — 文档变更

### Commit 格式

使用 Conventional Commits 前缀 + 中文描述：

```
feat: 添加麦克风设备选择功能
fix: 修复 Windows 托盘图标不显示
docs: 更新架构文档
ci: 添加文档格式检查
```

### PR 流程

1. 从 `main` 创建功能分支
2. 完成开发并确保所有检查通过
3. 提交 PR，描述变更内容和测试方法
4. 等待 CI 通过 + 代码审查

## 代码质量标准

项目执行硬阈值，超过必须拆分，无例外：

| 指标 | 阈值 |
|------|------|
| 单文件行数 | ≤ 800 行 |
| 单函数行数 | ≤ 30 行（含 JSX return） |
| 嵌套层级 | ≤ 3 层 |
| 分支数量 | ≤ 3 个/函数 |

## 测试

```bash
cargo test -p tingyuxuan-core    # Rust 核心测试
npm test                          # 前端测试（vitest）
npx tsc --noEmit                  # TypeScript 类型检查
npm run lint                      # ESLint
npm run lint:docs                 # Markdown 格式检查
```

## 文档约定

项目遵循**文档驱动开发（DDD）**和**唯一真值（SSOT）**原则：

- 每类信息只有一个权威来源，不在多处重复
- 功能改动必须同步更新对应模块文档（`docs/modules/`）
- 版本发布前检查 [Sprint 完成检查清单](docs/README.md)

完整文档体系导航见 [docs/README.md](docs/README.md)。

## 许可证

本项目采用 Source-Available 许可证，代码公开仅供参考和学习。详见 [LICENSE](LICENSE)。
