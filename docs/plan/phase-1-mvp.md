# Phase 1: MVP 核心骨架

**状态**: 已完成
**时间**: 2025-01
**Commit**: `db7c64e feat: initialize TingYuXuan MVP project structure`

## 目标

构建 Rust 核心引擎和 Tauri 后端骨架，验证技术方案可行性。

## 完成内容

### Rust Core (`crates/tingyuxuan-core/`)
- 音频录音模块（cpal + hound，16kHz mono WAV）
- STT Provider trait + WhisperProvider + DashScopeASRProvider
- LLM Provider trait + OpenAICompatProvider
- Pipeline 编排器（STT → LLM，带重试和取消）
- 配置管理（JSON 序列化，XDG 目录）
- 历史记录管理（SQLite）
- 提示词系统（4 种模式：Dictate/Translate/Edit/AiAssistant）
- 统一错误类型系统

### Tauri Backend (`src-tauri/`)
- 基础命令框架
- 窗口配置（FloatingBar + Settings）

### React Frontend (`src/`)
- 项目脚手架（React 19 + Zustand + Tailwind）

### 测试
- 62 个 Rust 单元测试通过

## 关键决策

- 选择 Tauri 2.0 → [ADR-0001](../architecture/adr/0001-tauri-framework.md)
