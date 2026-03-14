# Session Employee Name Projection Design

## Goal

让 `list_sessions/get_sessions` 返回更完整的会话数据，在员工直聊场景中直接携带 `employee_name`，使前端不必再次从员工列表反查。

## Current Behavior

- `create_session` 会把 `employee_id` 写入 `sessions` 表。
- `list_sessions/get_sessions` 会返回 `employee_id`、`display_title` 等字段。
- 后端在构造会话列表时已经读取了 `agent_employees`，并用它推导 `display_title`。
- 但会话 JSON 本身没有返回 `employee_name`，前端如果想稳定知道“当前会话属于哪个员工”，仍需额外映射。

## Proposed Change

在 `apps/runtime/src-tauri/src/commands/chat_session_io.rs` 的会话投影中，复用现有员工映射结果，直接把 `employee_name` 加入每条 session JSON：

- 员工直聊且能匹配到员工时，返回对应 `employee_name`
- 其余会话返回空字符串
- 保持现有 `display_title` 推导逻辑不变

## Why This Approach

- 改动最小，不扩大范围
- 利用现有 `agent_employees` 查询结果，不增加新的数据源复杂度
- 前端可自然消费 `employee_name`，减少补偿逻辑
- 与现有 `display_title` 语义互补：`display_title` 负责展示标题，`employee_name` 负责明确身份

## Validation

- 增加后端回归测试，验证员工直聊会话返回 `employee_name`
- 运行现有 `chat_session_io` 相关测试，确认 `display_title` 等既有行为不回归
