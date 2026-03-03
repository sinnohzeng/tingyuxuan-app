# Phase 7: Windows 语音 MVP 修复与重构（当前执行 SSOT）

## 背景与问题定位

目标是先把 Windows 端语音输入闭环稳定跑通：`RAlt 触发 -> 录音 -> 云端多模态处理 -> 注入结果`。

已验证事实（来自本机运行日志与仓库实现）：

- 当前链路并非“没有录音/没有请求云端”。录音、编码、LLM 请求都在发生。
- 用户观察到的“固定文案”主因是模型能力/配置不匹配：运行配置使用了 `qwen-turbo`，返回了模板化文本。
- 设置页展示与真实运行模型存在偏差，连接测试也仅覆盖“文本可连通”，未验证音频多模态能力。
- 现状编码为 WAV，尚未进入高压缩上传路径。

参考：

- `%APPDATA%/tingyuxuan/TingYuXuan/data/logs/tingyuxuan.log.*`
- `src-tauri/src/commands.rs`
- `crates/tingyuxuan-core/src/llm/multimodal.rs`
- `crates/tingyuxuan-core/src/pipeline/orchestrator.rs`

## 目标与非目标

### 目标

1. 彻底跑通最小可行产品（MVP）语音闭环，消除固定文案问题。
2. 交互层达到“可用且可感知”标准：启动过渡、实时波形、Thinking 状态。
3. 建立模型/连接/质量门槛，阻断“看起来成功但结果无效”的假阳性。
4. 落地 DDD + SSOT 文档同步机制，保证后续迭代可追溯。

### 非目标

1. 本阶段不实现 Typeless 同级上下文采集代码（UIA/OCR/截图等）。
2. 本阶段不落地 OPUS 直传；先实现 MP3 压缩与 WAV 回退。
3. 本阶段不做跨平台统一优化（优先 Windows）。

## 已锁定产品决策

1. 优先交付“录音转写最小 MVP”。
2. 模型由程序层统一固定，不开放给用户切换。
3. 模型更新策略：版本固定，随发版升级。
4. 编码策略：先 MP3 后 OPUS。
5. 上下文增强：本阶段只做技术方案和接口预留。

## MVP 实施阶段

### 阶段 A（P0）：正确性修复

1. 固定单一运行模型（推荐 `qwen-omni-turbo`，最终以发版时阿里云可用模型为准）。
2. 后端构建 Pipeline 时忽略用户可编辑模型字段，运行时只使用程序固定模型。
3. 设置页改为只读展示“当前运行模型”，删除误导性可配置暗示。
4. 连接测试升级为“多模态音频能力测试”。
5. 新增输出质量闸门：占位文本、空文本、明显模板响应直接报错并引导重试。

### 阶段 B（P0）：交互与状态机修复

1. 状态机扩展为：`idle -> starting -> recording -> thinking -> done/error/cancelled`。
2. 首次 RAlt 进入 `starting`（微加载），录音流准备完成后进入 `recording`。
3. 二次 RAlt 进入 `thinking`，浮窗显示 `Thinking.../思考中...`。
4. 防误触阈值从 800ms 下调至 250ms。
5. 浮窗尺寸与位置调整为贴近目标形态（紧凑尺寸、任务栏上方约 3-5px）。

### 阶段 C（P1）：音频压缩链路

1. 扩展编码器支持 MP3。
2. 上传默认 MP3；编码失败自动回退 WAV。
3. 记录编码指标：编码耗时、字节大小、压缩比、上传耗时。

### 阶段 D（P1）：Prompt 与结果稳定性

1. 重构 Dictate Prompt：强约束“识别 + 去口语化 + 结构化”。
2. 加入明确反模板规则，抑制“请开始录音”类伪结果。
3. 引入词典优先、序号列表自动化、重复修正策略。

### 阶段 E（P2）：上下文增强方案（仅设计）

1. 设计 `UIA + OCR` 混合采集架构。
2. 明确字段分级、采样频率、上传策略、性能预算。
3. 完成风险评估与合规说明，不在本阶段编码。

## 接口与类型变更（规划）

### Rust 后端

1. `PipelineEvent` 新增：`RecorderStarting`、`ThinkingStarted`。
2. 连接测试命令从 `test_llm_connection` 演进为 `test_multimodal_connection`。
3. 结构化错误新增：`model_not_multimodal`、`invalid_transcript_placeholder`。
4. 音频格式扩展：`wav | mp3`（并保留回退机制）。

### 前端 TypeScript

1. `RecordingState` 新增 `starting`、`thinking`。
2. 浮窗渲染与动效按新状态机改造。
3. 设置页 API 区只读显示“运行模型 + 连接状态”。

## 验收标准

### 自动化

1. Rust：模型守卫、多模态连接测试、输出质量门槛、编码回退路径测试通过。
2. Frontend：新增状态机与浮窗 UI 测试通过。
3. 现有测试基线持续通过（`cargo test -p tingyuxuan-core`、`npm test`）。

### 手工验证（Windows）

1. 按 RAlt 后立即出现微加载，随后波形实时变化。
2. 再按 RAlt 后显示 `Thinking...`，约 1-2 秒返回有效文本。
3. 文本自动注入目标输入框，无固定占位文案。
4. 日志可见完整链路：录音、编码、LLM、注入、耗时。
5. 失败路径可解释：给出可操作提示（重试/检查 API Key/麦克风）。

## 风险与回滚

1. 模型兼容性风险：固定模型若下线会导致失败。
2. 编码风险：MP3 编码兼容和性能波动。
3. 交互风险：悬浮窗尺寸过小导致可读性下降。

回滚策略：

1. 模型常量可快速回退到上一个稳定模型。
2. 编码路径可强制回退 WAV。
3. UI 状态机可回退到 `recording/processing` 双态显示。

## 文档同步矩阵（DDD / SSOT）

每次代码提交必须同步更新唯一权威文档，不做跨文档重复描述：

1. 录音/编码：`docs/modules/audio.md`
2. 模型/Prompt/多模态：`docs/modules/llm.md`
3. 状态机/事件流：`docs/modules/pipeline.md`
4. 交互时序与窗口：`docs/architecture/data-flow.md`
5. 用户说明与排障：`docs/guides/usage.md`、`docs/guides/troubleshooting.md`
6. 当前执行计划与里程碑：`docs/plan/phase-7-voice-mvp-remediation.md`

## 执行顺序与里程碑

1. M1（P0）：正确性闭环跑通（固定模型 + 质量门槛 + 新连接测试）
2. M2（P0）：交互状态机达标（starting/recording/thinking）
3. M3（P1）：压缩链路可用（MP3 默认 + WAV 回退）
4. M4（P1）：Prompt 稳定与文案质量提升
5. M5（P2）：上下文增强设计评审完成（仅方案）

## 执行进展（2026-03-03）

- [x] M1：运行模型固定为 `qwen3-omni-flash`；连接测试升级为 `test_multimodal_connection`（真实音频探测）；新增模板占位文本质量闸门
- [x] M2：事件/状态机落地 `idle -> starting -> recording -> thinking -> done|error|cancelled`；防误触阈值 250ms；浮窗尺寸与位置更新（220x56，任务栏上方约 4px）
- [x] M3：编码链路改为 MP3 优先、WAV 回退；并支持服务器拒绝 MP3 时自动回退 WAV 重试
- [x] M4：Prompt 重构（反模板约束、去口语化、结构化输出、词典优先）
- [x] M5：完成上下文增强接口预留与方案边界定义（Windows 侧保持 UIA/OCR 设计位，不在本阶段实现）
- [x] MVP 录音时长治理：限制 `<=5 分钟`，第 4 分钟起显示 60 秒倒计时和限制 Pop up，5 分钟自动停止并进入 Thinking

## 外部调研依据（用于方案参考）

1. Typeless 官网：<https://www.typeless.com/>
2. Typeless Data Controls：<https://www.typeless.com/data-controls>
3. Typeless Quick Start：<https://www.typeless.com/help/quickstart/first-dictation>
4. Typeless Release Notes：<https://www.typeless.com/help/release-notes>
5. Typeless Privacy Policy：<https://www.typeless.com/privacy>
6. 阿里云模型列表：<https://help.aliyun.com/zh/model-studio/model-list>
7. 阿里云 Realtime API：<https://help.aliyun.com/zh/model-studio/realtime-api>
8. 阿里云 Qwen-Omni：<https://help.aliyun.com/zh/model-studio/qwen-omni>
