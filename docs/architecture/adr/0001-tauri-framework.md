# ADR-0001: 选择 Tauri 2.0 作为桌面应用框架

**状态**: Accepted
**日期**: 2025-01 (Phase 1)

## 背景

听语轩需要一个跨平台桌面应用框架，支持 Linux、macOS 和 Windows。核心需求包括：
- 系统托盘集成
- 全局快捷键
- 透明无边框浮动窗口（Always-on-top）
- 系统级文本注入（调用平台 CLI 工具）
- 小型安装包（面向个人用户）

## 决策

选择 Tauri 2.0 + React 前端。

Tauri 2.0 关键优势：
- **安全性**：Rust 后端，WebView 沙箱，细粒度权限（capabilities）
- **包体积**：~5-10MB（vs Electron ~100MB+），使用系统 WebView
- **Rust 生态**：核心引擎直接用 Rust 编写（cpal 音频、reqwest HTTP、rusqlite 数据库），无需 FFI
- **跨平台**：支持 Linux (WebKitGTK)、macOS (WebKit)、Windows (WebView2)
- **插件系统**：tauri-plugin-global-shortcut、tauri-plugin-shell 等

## 后果

**正面**：
- 核心引擎（`crates/tingyuxuan-core/`）完全平台无关，可独立测试
- 包体积远小于 Electron 方案
- Rust 类型系统和所有权模型提升代码安全性

**负面**：
- Tauri 2.0 生态相比 Electron 较新，社区资源较少
- WebView 在不同平台上的行为差异（CSS 渲染、性能）
- Wayland 上全局快捷键支持受限（compositor 依赖）

## 备选方案

| 方案 | 未选择原因 |
|------|-----------|
| Electron | 包体积过大（100MB+），内存占用高，安全模型较弱 |
| 原生 GUI (GTK/Qt) | 开发效率低，跨平台 UI 一致性差 |
| Flutter Desktop | Rust 集成复杂，FFI 开销 |
