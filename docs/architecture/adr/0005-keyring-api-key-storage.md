# ADR-0005: OS Keyring API Key 存储

**状态**: Accepted
**日期**: 2025-02 (Phase 2)

## 背景

听语轩需要存储用户的 STT 和 LLM API Key。这些 Key 具有商业价值（按调用量计费），泄露会导致用户财务损失。需要一个安全的存储方案。

## 决策

使用 **keyring crate** 通过操作系统密钥管理服务存储 API Key，并提供明文降级方案。

存储层级：
1. **首选**：OS Keyring（Linux: libsecret/GNOME Keyring, macOS: Keychain, Windows: Credential Manager）
   - service: `"tingyuxuan"`, username: `"stt_api_key"` / `"llm_api_key"`
2. **降级**：当 keyring 不可用时（无图形环境的 headless 服务器），API Key 存储在配置文件的 `api_key_ref` 字段中

读取逻辑（`resolve_api_key`）：
1. 尝试从 keyring 读取
2. 如果 keyring 失败 → 读取 `config.api_key_ref`
3. 如果 ref 以 `@keyref:` 开头 → 表示"仅在 keyring 中"，返回 None

## 后果

**正面**：
- API Key 不以明文存储在磁盘文件中（keyring 可用时）
- 利用 OS 原生安全机制（加密存储、进程隔离）
- 降级方案确保在 headless 环境（CI、开发服务器）也能工作
- 用户无感知——设置界面自动尝试 keyring

**负面**：
- keyring crate 在 Linux 上依赖 libsecret（需要 D-Bus），某些极简 Linux 发行版可能缺失
- 降级到明文存储时安全性降低（但有文件系统权限保护）
- 无法在 keyring 不可用时自动提示用户

## 备选方案

| 方案 | 未选择原因 |
|------|-----------|
| 仅明文配置文件 | 安全性不足，Key 可被其他进程读取 |
| 加密配置文件 | 需要管理加密密钥，增加复杂度 |
| 环境变量 | 用户体验差，每次启动需要设置 |
| Tauri 安全存储插件 | Tauri 2.0 尚无官方安全存储插件 |
