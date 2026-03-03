# 听语轩 TingYuXuan

AI 驱动的智能语音输入工具 —— 将口语化的语音自动转为规范的书面文字。

[![CI](https://github.com/sinnohzeng/tingyuxuan-app/actions/workflows/ci.yml/badge.svg)](https://github.com/sinnohzeng/tingyuxuan-app/actions/workflows/ci.yml)

## 特性

- **语音转润色文字**：多模态 LLM 一步识别+润色，输出即发送级文本
- **四种模式**：听写、翻译、AI 助手、编辑已选文本
- **全局输入**：在任何应用中按快捷键即可使用
- **配置简单**：仅需一个 DashScope API Key，无需独立 STT 服务
- **安全存储**：API Key 存储在 OS Keyring 中
- **离线可靠**：网络断开时录音自动排队，恢复后自动处理
- **隐私优先**：所有 API 调用在本地 Rust 后端发起，零数据留存

## 快捷键

| 快捷键 | 功能 |
|-------|------|
| `RAlt` | 听写模式 |
| `Shift+RAlt` | 翻译模式 |
| `Alt+Space` | AI 助手 |
| `Esc` | 取消录音 |

> **注意**: `Alt+Space` 在 Windows 上是系统快捷键（窗口菜单），如被系统拦截可在设置中自定义。Linux 下 RAlt 可能被配置为 Compose 键，需在系统设置中调整。

## 安装

### 下载安装包

从 [Releases](https://github.com/sinnohzeng/tingyuxuan-app/releases) 下载最新版本：

- `.deb` — Ubuntu / Debian
- `.AppImage` — 通用 Linux
- `.msi` / `.exe` — Windows
- `.dmg` — macOS
- `.apk` — Android

### 从源码编译

```bash
# 安装系统依赖（Ubuntu）
sudo apt install libasound2-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

# 安装文本注入工具
sudo apt install xdotool xclip  # X11
# 或
sudo apt install wtype wl-clipboard  # Wayland

# 构建
git clone https://github.com/sinnohzeng/tingyuxuan-app.git
cd tingyuxuan-app
npm install
npx tauri build
```

## 技术栈

| 层 | 技术 |
|----|------|
| 核心 | Rust 2024 (cpal, reqwest, rusqlite) |
| 应用框架 | Tauri 2.10 |
| 前端 | React 19 + TypeScript + Tailwind CSS 4 + Zustand 5 |
| UI 组件库 | Fluent UI 2 (@fluentui/react-components) |
| 测试 | 222 自动化测试（Rust + vitest + JNI + Android） |

## 文档

详细文档位于 [`docs/`](docs/README.md)：

- [产品需求文档（PRD）](docs/prd.md)
- [系统架构](docs/architecture/overview.md)
- [安装指南](docs/guides/installation.md)
- [使用指南](docs/guides/usage.md)
- [配置指南](docs/guides/configuration.md)
- [故障排查](docs/guides/troubleshooting.md)

## 许可证

Source-Available，保留所有权利。详见 [LICENSE](LICENSE)。
