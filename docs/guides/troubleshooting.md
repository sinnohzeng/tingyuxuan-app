# 故障排查

## 常见问题

### 1. 文本无法注入到应用中

**症状**：录音和处理成功，但文字没有出现在目标应用中。

**排查步骤**：

1. 检查是否安装了文本注入工具：
   ```bash
   # X11
   which xdotool && which xclip

   # Wayland
   which wtype && which wl-clipboard
   ```

2. 确认显示服务器类型：
   ```bash
   echo $XDG_SESSION_TYPE  # 应该输出 x11 或 wayland
   ```

3. 安装缺失的工具（参见 [安装指南](installation.md)）

### 2. Wayland 下全局快捷键不工作

**原因**：Wayland 协议限制了应用注册全局快捷键的能力。部分 Wayland 合成器（如 GNOME Mutter）可能不支持。

**解决方案**：
- 使用支持 `wlr-protocols` 的合成器（如 Sway、Hyprland）
- 或在桌面环境的快捷键设置中手动绑定命令

### 3. API Key 保存失败

**症状**：保存 API Key 后提示错误，或重启后 Key 丢失。

**原因**：操作系统 keyring 服务未运行。

**排查步骤**：

```bash
# 检查 Secret Service 是否运行
dbus-send --session --print-reply \
  --dest=org.freedesktop.secrets \
  /org/freedesktop/secrets \
  org.freedesktop.DBus.Peer.Ping
```

**解决方案**：
- GNOME：确保 `gnome-keyring-daemon` 正在运行
- KDE：确保 `kwallet` 已启用
- 无桌面环境：API Key 会降级存储到配置文件明文中

### 4. 录音没有声音 / 识别结果为空

**排查步骤**：

1. 检查麦克风权限：
   ```bash
   arecord -l  # 列出可用的录音设备
   ```

2. 确认当前用户在 `audio` 组中：
   ```bash
   groups | grep audio
   ```

3. 测试录音功能：
   ```bash
   arecord -d 3 test.wav && aplay test.wav
   ```

### 5. 网络错误 / API 调用失败

**常见错误码**：

| 错误 | 说明 | 解决方案 |
|------|------|---------|
| 401 Unauthorized | API Key 无效 | 检查并重新输入 API Key |
| 429 Too Many Requests | 请求频率超限 | 稍后重试 |
| 500 Server Error | 服务端错误 | 检查 Provider 服务状态 |
| Connection Error | 网络不可达 | 检查网络连接；录音会自动加入离线队列 |

**使用「测试连接」功能**：在设置中点击 LLM 的测试连接按钮，快速诊断问题。

### 6. Windows：文本注入到部分应用无效

**症状**：文本注入在大多数应用正常工作，但在某些管理员权限运行的程序中无效。

**原因**：Windows UIPI（User Interface Privilege Isolation）阻止低权限进程向高权限窗口发送输入事件。

**解决方案**：
- 以管理员身份运行听语轩
- 或避免在管理员权限的应用中使用文本注入

### 7. Windows：WebView2 未安装

**症状**：Windows 10 上启动应用后出现错误提示。

**解决方案**：
- 下载安装 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
- Windows 11 自带 WebView2，无需安装

### 8. 应用启动后白屏

**可能原因**：前端渲染崩溃。

**解决方案**：
- 应用内置了 Error Boundary，通常会显示错误信息和「重试」按钮
- 如果仍然白屏，尝试删除配置文件后重启：
  ```bash
  rm ~/.config/tingyuxuan/TingYuXuan/config.json
  ```

### 9. ydotool 文本注入需要权限

如果使用 `ydotool` 作为备用输入工具：

```bash
# 将用户加入 input 组
sudo usermod -aG input $USER
# 重新登录后生效
```

---

## 日志查看

听语轩使用 `tracing` 框架输出日志。设置环境变量可查看详细日志：

```bash
RUST_LOG=debug tingyuxuan-app
```

日志级别：`error` < `warn` < `info` < `debug` < `trace`

---

## 数据目录

### Linux

| 目录 | 内容 |
|------|------|
| `~/.config/tingyuxuan/TingYuXuan/` | 配置文件 |
| `~/.local/share/tingyuxuan/TingYuXuan/` | 历史数据库、离线队列、音频缓存 |

### Windows

| 目录 | 内容 |
|------|------|
| `%APPDATA%\com.tingyuxuan.app\` | 配置文件 |
| `%LOCALAPPDATA%\com.tingyuxuan.app\` | 历史数据库、离线队列、音频缓存 |

---

## 反馈与报告问题

如果以上方法无法解决你的问题，请在 GitHub 上提交 Issue：

https://github.com/sinnohzeng/tingyuxuan-app/issues

提交时请附带：
- 操作系统版本（Windows 10/11 版本号 或 Linux 发行版）
- 显示服务器类型（X11 / Wayland，仅 Linux）
- 错误信息截图或日志输出
