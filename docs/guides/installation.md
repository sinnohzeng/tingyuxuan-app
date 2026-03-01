# 安装指南

## 系统要求

### Windows

- **操作系统**：Windows 10 (1809+) 或 Windows 11
- **架构**：x86_64
- **运行时**：WebView2（Windows 11 自带；Windows 10 需安装 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)）
- **音频**：Windows 兼容的麦克风

### Linux

- **操作系统**：Ubuntu 22.04+、Fedora 38+ 或其他支持 WebKitGTK 的发行版
- **架构**：x86_64
- **音频**：ALSA 兼容的麦克风
- **显示**：X11 或 Wayland

### 必需的系统工具

听语轩使用以下工具进行文本注入（至少需要一组）：

| 显示服务器 | 键入工具 | 剪贴板工具 |
|-----------|---------|-----------|
| X11 | `xdotool` | `xclip` |
| Wayland | `wtype` | `wl-clipboard` |

```bash
# Ubuntu / Debian (X11)
sudo apt install xdotool xclip

# Ubuntu / Debian (Wayland)
sudo apt install wtype wl-clipboard

# Fedora (X11)
sudo dnf install xdotool xclip

# Fedora (Wayland)
sudo dnf install wtype wl-clipboard
```

---

## 安装方式

### Windows

#### 方式一：NSIS 安装程序（推荐）

从 [Releases 页面](https://github.com/sinnohzeng/tingyuxuan-app/releases) 下载最新 `.exe` 安装程序：

1. 运行 `TingYuXuan_0.2.0_x64-setup.exe`
2. 选择安装语言（简体中文 / English）
3. 按向导完成安装

#### 方式二：MSI 安装包

适合企业部署和组策略分发：

```powershell
msiexec /i TingYuXuan_0.2.0_x64_en-US.msi
```

### Linux

#### 方式一：.deb 包（推荐 — Ubuntu/Debian）

从 [Releases 页面](https://github.com/sinnohzeng/tingyuxuan-app/releases) 下载最新 `.deb` 文件：

```bash
sudo dpkg -i tingyuxuan_0.2.0_amd64.deb
sudo apt-get install -f  # 安装缺失的依赖
```

#### 方式二：AppImage（通用 Linux）

```bash
chmod +x TingYuXuan_0.2.0_amd64.AppImage
./TingYuXuan_0.2.0_amd64.AppImage
```

### 从源码编译

**前置要求**：

- Rust 工具链（`rustup`）
- Node.js 20+
- 系统依赖：

```bash
# Ubuntu / Debian
sudo apt install libasound2-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

**编译步骤**：

```bash
git clone https://github.com/sinnohzeng/tingyuxuan-app.git
cd tingyuxuan-app
npm install
npm install -g @tauri-apps/cli
npx tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`。

---

## 首次运行

1. 启动听语轩后，应用会在系统托盘中运行
2. 首次启动时会自动打开设置窗口
3. 配置 LLM API Key（参见 [配置指南](configuration.md)）
4. 配置完成后，使用快捷键即可开始语音输入
