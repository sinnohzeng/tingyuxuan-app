# 2026-03-03 MVP 整体验收报告

## 验收范围

本次验收覆盖以下目标：

1. Phase 7 语音 MVP 修复项是否可构建、可测试、可发布。
2. 新增规则是否生效：
   - 单次录音时长限制 `<= 5 分钟`
   - 第 4 分钟触发 60 秒倒计时与时长提示 Pop up
   - 5 分钟自动停止并进入 Thinking
3. Windows 侧 Typeless 风格录音浮窗是否完成实现（胶囊 + 上下对称波形 + 左右圆形按钮）。

## 自动化验收命令与结果

执行日期：2026-03-03（Asia/Shanghai）

1. `npm test`  
   结果：通过（16 files, 74 tests passed）
2. `cargo test -p tingyuxuan-core --quiet`  
   结果：通过（136 passed）
3. `cargo check --workspace --quiet`  
   结果：通过（仅告警，无错误）
4. `npm run build`  
   结果：通过（前端构建成功）
5. `cargo test -p tingyuxuan-app --quiet`  
   结果：通过（18 passed）

## 问题修复记录（验收过程中）

1. **Windows 剪贴板单测偶发失败**  
   现象：`test_clipboard_roundtrip_ascii` 偶发读回空字符串。  
   修复：在测试中加入短时重试（20 次）与轻量退避，避免剪贴板争用导致的瞬时空读。  
   文件：`src-tauri/src/platform/windows.rs`

## 验收结论

1. 自动化验收通过，可以进入发布与 CI 阶段。
2. 录音时长治理（4 分钟倒计时 + 5 分钟自动截止）已进入代码、测试和文档三处一致状态。
3. 当前 MVP 明确只支持 `qwen3-asr-flash`，不处理超过 5 分钟音频，不启用 filetrans 分流。

## 手工验收建议（发布前）

1. Windows 真机按 `RAlt` 两次，验证浮窗位置与 4 分钟倒计时提示。
2. 连续录音到 5 分钟，确认自动停录后能正常进入 Thinking 并产出结果。
3. 在目标输入框验证文本注入（普通输入框 / 富文本输入框至少各 1 个）。
