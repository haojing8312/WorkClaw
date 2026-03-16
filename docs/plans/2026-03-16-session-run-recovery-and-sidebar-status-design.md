# 会话恢复与侧栏状态设计

**日期：** 2026-03-16

## 背景

当前多会话并行时存在两个体验问题：

1. 某个会话正在流式执行时，用户切回首页并新开会话，再返回旧会话，会看到从“上一次用户输入之后”开始的部分 assistant 输出丢失。随后如果旧会话仍在继续执行，新 token 又会继续展示，形成“前半段丢了、后半段继续长出来”的断裂体验。
2. 侧栏会话列表缺少一眼可读的运行状态，用户在并行多个任务时无法快速分辨哪些会话正在执行、哪些在等待确认、哪些已经完成或失败。

## 问题分析

### 1. 会话内容丢失的根因

`ChatView` 在切换 `sessionId` 时会重置本地流式状态，并调用 `get_messages(sessionId)` 重新加载历史消息。这个接口只返回已经落入 `messages` 表的正式消息。

对于仍在运行中的一次会话执行：

- 中途生成的文本会持续累加到 `session_runs.buffered_text`
- 但在 run 完成并绑定 `assistant_message_id` 之前，这段内容通常还不是一条正式 assistant message
- 因此当用户重新进入该会话时，前端先看到的是数据库里的旧快照，只保留“上一条 user message”
- 随后实时事件继续推送，新 token 会接在空白状态后继续出现，于是形成内容断层

这不是流式事件本身丢失，而是“会话恢复时没有把运行中投影重新渲染出来”。

### 2. 侧栏状态缺失的根因

`list_sessions` 目前只返回基础会话元数据，不返回会话最近一次运行的聚合状态。侧栏只能展示标题、来源和操作按钮，无法稳定呈现运行态。如果让前端自己遍历 `list_session_runs` 推导，会带来多次请求、状态来源分散和列表/详情不一致的问题。

## 目标

1. 用户在任意时刻切走再切回运行中的会话，不应丢失任何已生成内容。
2. 侧栏为每个会话显示轻量状态图标，至少支持：
   - `running`
   - `waiting_approval`
   - `completed`
   - `failed`
3. 会话详情页和侧栏使用同一套运行状态来源，避免推断口径不一致。
4. 不引入全量事件回放架构，优先在现有 `session_runs` 投影上补齐恢复能力。

## 方案选择

最终采用方案 B：后端投影补强 + 前端恢复层。

### 不采用方案 A（纯前端兜底）的原因

- 需要前端自己拼接和猜测当前状态
- 侧栏如需状态展示，要么为每个会话额外查 runs，要么在前端维护复杂推导逻辑
- 后续再增加状态种类时，列表与详情更容易出现不一致

### 不采用方案 C（事件流完全回放）的原因

- 架构成本明显高于本次问题规模
- 当前已有 `session_runs` 和 `session_run_events` 投影，可先满足恢复与状态展示需求

## 设计

### 一、运行中内容恢复

在 `ChatView` 切换会话时，除加载 `messages` 外，再读取 `list_session_runs(sessionId)` 返回的 run 投影，识别“尚未落成正式 assistant message 的最新活跃 run”。

判定逻辑：

- run 状态为 `thinking`、`tool_calling` 或 `waiting_approval`
- `assistant_message_id` 为空
- `buffered_text` 非空，或已有工具调用/审批状态可展示

当前端满足上述条件时，构造一条仅用于渲染的恢复态 assistant 消息：

- 不写入 `messages` 表
- 不污染正式消息顺序
- 作为消息列表尾部的临时项显示
- 后续若流式 token 继续到来，则在这条恢复态消息基础上续写
- 若 run 完成并产生正式 assistant message，则用真实消息替换恢复态消息

这样用户重新进入会话时，不会先退回到“上一条 user message”。

### 二、会话运行状态聚合

在后端 `list_sessions` 查询中，为每个 session 聚合一个轻量运行状态字段，例如：

- `runtime_status`

推荐映射规则：

1. 若该 session 存在最新未终结 run：
   - `waiting_approval` 优先级最高
   - 否则 `thinking` / `tool_calling` 统一映射为 `running`
2. 若不存在未终结 run，则查看最近一条 run：
   - `completed` -> `completed`
   - `failed` / `cancelled` -> `failed`
3. 若从未执行过 run，则 `runtime_status` 为空

这样列表只消费聚合结果，不自己猜状态。

### 三、侧栏图标设计

侧栏会话项标题左侧增加固定状态图标：

- `running`：蓝色旋转加载图标
- `waiting_approval`：琥珀色提醒图标
- `completed`：绿色对勾图标
- `failed`：红色错误图标

交互约束：

- 图标始终保留，不因 hover 操作按钮而消失
- 当前选中会话保留状态色，不因选中背景而丧失可辨识度
- tooltip 显示中文文案：`执行中`、`等待确认`、`已完成`、`执行失败`
- 无运行历史的会话不展示图标，避免侧栏噪音过多

### 四、详情页与侧栏的一致性

`ChatView` 内部继续使用 `sessionRuns` 做失败卡片、审批提示和恢复态判断；侧栏仅依赖 `SessionInfo.runtime_status`。两者均来自同一张 `session_runs` 投影表，但职责不同：

- 详情页：使用完整 run 列表做精细恢复与失败展示
- 侧栏：使用聚合状态做轻量扫描

这样既保持一致，又不会把侧栏做得过重。

## 数据模型调整

前端 `SessionInfo` 新增：

- `runtime_status?: "running" | "waiting_approval" | "completed" | "failed" | string`

后端 `list_sessions_with_pool` 返回值新增：

- `runtime_status`

不需要新增数据库字段，直接在查询时聚合即可。

## 错误处理

1. 若 `list_session_runs` 加载失败，`ChatView` 仍展示正式消息，但不会显示恢复态消息。
2. 若 `runtime_status` 聚合失败，`list_sessions` 可返回空状态，侧栏不显示图标，避免阻断列表加载。
3. 若 run 已落成正式 assistant message，则恢复态逻辑必须跳过，避免重复显示。

## 测试策略

### 前端

新增或扩展测试覆盖：

1. 切回会话时，如果存在 `assistant_message_id` 为空且 `buffered_text` 非空的活跃 run，应显示恢复态 assistant 内容。
2. 活跃 run 完成并返回正式 assistant message 后，恢复态内容应消失，只保留正式消息。
3. 侧栏根据 `runtime_status` 渲染正确图标与文案。
4. 无 `runtime_status` 的会话不显示状态图标。

### 后端

新增或扩展测试覆盖：

1. `list_sessions_with_pool` 对 `thinking` / `tool_calling` 聚合为 `running`
2. `waiting_approval` 优先级高于普通运行态
3. 最近一次 run 为 `completed` 时返回 `completed`
4. 最近一次 run 为 `failed` / `cancelled` 时返回 `failed`
5. 没有 run 的会话返回空状态

## 风险与边界

1. 如果同一 session 理论上存在多个未终结 run，聚合逻辑必须只选择“最近活跃的一条”作为会话当前状态。
2. 恢复态消息只解决“已进入 run 投影但未落成正式消息”的缺口，不试图回放所有瞬时 UI 状态。
3. 这次不扩展到 `waiting_user`、`partial` 等更细状态，保持与当前用户确认的简洁状态集合一致。

## 预期结果

实施完成后：

- 用户切回旧会话时，不会再看到中途生成的内容消失
- 并行会话列表可以通过图标快速判断当前执行情况
- 列表与详情页围绕同一套后端投影工作，后续扩展状态时改动边界清晰
