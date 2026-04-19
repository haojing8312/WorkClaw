# OpenClaw IM Host：目标架构

本文档描述 WorkClaw 对齐 OpenClaw IM 插件生态后的目标分层、职责边界和核心数据流。

## 目标分层

### 1. OpenClaw IM Plugin Layer

这一层由官方或兼容 OpenClaw 的 IM 插件组成，例如飞书、企业微信。

职责：

- 监听渠道入站事件
- 承担 OpenClaw 官方 reply lifecycle
- 执行 typing / reaction / chunk send / final deliver
- 将标准化 dispatch 请求交给宿主
- 将发送结果与 lifecycle 状态回传给宿主

### 2. WorkClaw OpenClaw Host Layer

这是 WorkClaw 新增或重构后的宿主适配层，是本次重构的核心。

职责：

- 管理 IM plugin runtime 生命周期
- 接收插件 dispatch request 并路由到 WorkClaw runtime
- 将 WorkClaw runtime 的结果转为 OpenClaw-compatible reply plan
- 跟踪整轮 reply 的 chunk、状态与 completion
- 维护 target mapping、pairing、runtime registry 与 delivery trace

### 3. WorkClaw Core Runtime

保持现有核心价值不变。

职责：

- session / agent / workflow / employee / approval / ask_user
- 本地持久化与恢复
- 桌面工作台能力
- 业务逻辑执行

### 4. Frontend

前端只保留展示与用户交互，不再承担 IM reply orchestration。

职责：

- 展示 session、员工、审批、诊断信息
- 本地用户操作与桌面联动
- 查看 IM runtime 状态与 delivery trace

非职责：

- 不再直接回推 Feishu 最终答复
- 不再做 IM fallback poll
- 不再做任何渠道文本截断、补发或重试编排

## 核心能力

### Gateway

Gateway 保留，但职责收束。

负责：

- 入站事件归一化
- channel -> session / role / employee 路由
- direct/group/thread 解析
- pairing 与 sender/chat/thread 映射
- ask_user / approval / interrupt 的宿主桥接入口

不负责：

- UI fallback
- 文本裁切
- 私有最终回复补发

### Reply Lifecycle

宿主与插件共同确认整轮 reply 的生命周期：

1. reply started
2. chunk plan created
3. plugin dispatch / typing active
4. chunk deliveries
5. waitForIdle barrier
6. final completion
7. success / failed_partial / failed

完成条件必须是“整轮 reply 完成”，而不是第一条 `send_result` 返回。

### Chunk Planner

长文本分块应成为宿主平台能力。

要求：

- 渠道上限可配置
- 分块不丢字
- 可保留 role prefix
- 可记录 chunk index 与 trace
- 可被飞书、企业微信等渠道复用

### Delivery Trace

每次 logical reply 都应有独立 trace，至少包含：

- channel
- account_id
- session_id
- logical_reply_id
- target
- planned chunk count
- delivered chunk count
- failure details
- final state

## 目标数据流

### 入站

1. IM 插件接收消息
2. 插件/gateway 生成标准 dispatch request
3. WorkClaw host 接收并路由到 session/runtime
4. WorkClaw runtime 开始执行

### 出站

1. WorkClaw runtime 产出 final / ask_user / approval / failure
2. WorkClaw host 生成 reply plan
3. plugin layer 按 OpenClaw lifecycle 执行 typing / chunk / final deliver
4. plugin 向宿主回传 lifecycle 与 delivery result
5. 宿主更新 trace 与状态

## 设计约束

- WorkClaw 不重写 OpenClaw IM 生命周期，只负责宿主适配
- 渠道差异只留在 adapter 层
- 前端不参与 IM lifecycle 完成判定
- 任何 reply completion 都必须有可观测状态

## 预期收益

- 行为与 OpenClaw 官方插件更一致
- processing reaction / typing 可恢复
- 飞书长文本截断与 partial delivery 更易定位和治理
- 企业微信等渠道可复用同一宿主 contract
- 后续上游 vendor sync 有明确边界
