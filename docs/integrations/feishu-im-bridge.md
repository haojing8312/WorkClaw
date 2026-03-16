# 飞书 IM 闭环桥接（桌面会话 <-> 飞书群）

本文档说明运行时如何把智能体员工在桌面端的执行过程，持续桥接回飞书群，并在飞书侧完成 `ask_user` 与高风险审批闭环。

## 事件闭环

1. 飞书入站消息触发 `im-role-dispatch-request`
2. 桌面端 `App.tsx` 收到后：
   - 普通阶段调用 `send_message`
   - `ask_user` 等待阶段调用 `answer_user_question`
3. 执行过程中的 `stream-token` 被聚合后转发为 `send_feishu_text_message`
4. 收到 `ask-user-event` 时，立即把澄清问题发回飞书群
5. 用户在飞书回复后再次进入 `im-role-dispatch-request`，并回填到 `answer_user_question`

对应实现入口：`apps/runtime/src/App.tsx`

## 高风险审批闭环

当桌面运行时启用 `approval_bus_v1` 时，高风险工具不再依赖单次桌面弹窗，而是走统一审批总线：

1. `executor` 创建 pending approval，并把状态写入 `approvals`
2. 桌面收到 `approval-created` / `approval-resolved` 事件，展示审批队列
3. 同一条审批会同步转发到飞书线程，消息内包含 `/approve <approvalId> allow_once|allow_always|deny` 指令
4. 桌面或飞书任一端审批后，`ApprovalManager` 以数据库 CAS 方式终态化该 approval
5. 当前运行中的 agent 继续执行；若应用已重启，则启动恢复 bootstrap 读取 `approved AND resumed_at IS NULL` 的记录补执行

当前边界：

- 桌面与飞书共享同一个 `approvalId`
- `allow_once` 只放行当前一次
- `allow_always` 会生成结构化 `approval_rules`
- `deny` 会终止当前危险工具调用，但不会生成长期拒绝规则

## Rollout 开关与降级路径

审批总线默认开启；如果需要临时降级到旧的桌面确认路径，可在 `app_settings` 中写入：

```sql
INSERT OR REPLACE INTO app_settings (key, value) VALUES ('approval_bus_v1', 'false');
```

关闭后行为如下：

- `executor` 对高风险工具回退到旧的桌面 `tool-confirm-event + confirm_tool_execution` 路径
- 飞书 `/approve` 与审批通知不再参与新的高风险审批流
- 启动时不会执行 `approved AND resumed_at IS NULL` 的审批恢复 bootstrap

重新开启可将值改回 `true`，或直接删除该配置项。

## 流式转发策略（防刷屏 + 保实时）

- 文本聚合阈值：`STREAM_CHUNK_SIZE = 120`
- 时间节流窗口：`STREAM_FLUSH_INTERVAL_MS = 1200`
- 单条飞书消息最大长度：`1800`

策略说明：

- token 持续进入缓冲区，达到阈值或定时器到点会 flush
- 连续 flush 受 1200ms 窗口节流，避免高频调用飞书发送接口
- `done=true` 与 `ask-user-event` 会强制 flush，确保关键节点即时可见

## 子智能体（sub-agent）可见性

为支持“项目经理委派开发团队”场景，`sub_agent=true` 的流式 token 也参与飞书桥接，不再被忽略。

这保证了：

- 桌面端出现委派流式输出时，飞书端也有同步进度
- 飞书用户不会只在最后一步才看到回包

## 自动化覆盖

核心回归测试：`apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx`

覆盖点：

- `ask_user` 提问转发 + 后续回复走 `answer_user_question`
- 流式 token 转飞书
- `sub_agent` token 转飞书
- 节流窗口内不高频发送，窗口后补刷
- “委派流式 -> 需求澄清 -> 用户回复”闭环

执行命令：

```bash
pnpm --dir apps/runtime test -- App.im-feishu-bridge.test.tsx
```

## 常见问题排查

1. 飞书端完全无流式输出
   - 检查是否收到 `stream-token` 事件
   - 检查 `send_feishu_text_message` 调用是否成功
2. 只在结束时才看到一条消息
   - 检查是否处于节流窗口（默认 1200ms）
   - 检查 token 是否持续写入缓冲区
3. 澄清问题只弹桌面 UI，不回飞书
   - 检查 `ask-user-event` 是否命中 IM 桥接 session
   - 检查 `suppressAskUserPrompt` 是否仅影响桌面提示，不影响飞书转发
