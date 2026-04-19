# OpenClaw IM Host：第三阶段实施计划

第三阶段的目标是把飞书改造结果抽象成通用 IM 宿主框架，让企业微信及后续更多 IM 都按同一 OpenClaw-compatible 模式接入。

## 阶段目标

- 抽象通用 IM host framework
- 把飞书收敛为 adapter，而不是特例子系统
- 让企业微信复用同一 host contract
- 建立多渠道 runtime registry、delivery trace 与测试矩阵

## 任务清单

### 1. 抽象通用 IM host framework

建议新增目录：

- `apps/runtime/src-tauri/src/commands/im_host/`

建议模块：

- `contract.rs`
- `lifecycle.rs`
- `delivery_trace.rs`
- `chunk_planner.rs`
- `target_resolver.rs`
- `runtime_registry.rs`

### 2. 收敛 FeishuGateway 为 adapter

目标：

- `feishu_gateway` 只保留飞书特有规则
- 通用 lifecycle、trace、chunk、runtime 管理抽到 `im_host`

### 3. 企业微信按同模式接入

原则：

- 不再重复飞书当前的私有桥接路径
- 直接走统一 host contract
- ask_user / approval / final answer 全部复用通用框架

### 4. 建立统一 runtime registry

统一管理：

- 渠道类型
- runtime 状态
- account 与 health
- capability 与 diagnostics

### 5. 抽象统一 target model

统一概念：

- conversation target
- thread target
- reply-to target
- direct user target

各渠道 adapter 只做字段翻译，不再让业务层散写 target 解析。

### 6. 统一 delivery trace

统一记录：

- channel
- account
- logical_reply_id
- target
- lifecycle transition
- chunk delivery
- final state

### 7. 统一前端 IM 管理视图

前端只展示：

- 渠道 runtime 状态
- pairing / account 信息
- delivery trace 与 diagnostics

不再参与渠道 reply orchestration。

### 8. 建立双层测试矩阵

通用层：

- lifecycle order
- chunk integrity
- partial failure
- ask_user / approval bridge

渠道层：

- Feishu-specific
- WeCom-specific

### 9. 定义插件兼容等级

建议定义：

- L1：基础收发
- L2：完整 reply lifecycle
- L3：ask_user / approval / processing state
- L4：diagnostics / recovery / vendor sync compatibility

### 10. 固化 vendor sync 机制

明确：

- 哪些目录跟随 upstream 行为
- 哪些模块是 WorkClaw adapter
- 上游升级时的回归边界与流程

## 验收标准

- 飞书成为通用 IM host framework 的一个 adapter
- 企业微信可以复用同一宿主 contract
- lifecycle / chunk / trace 成为平台能力
- WorkClaw 具备长期兼容 OpenClaw IM 插件生态的结构基础

## 当前进展

截至当前实现，Phase 3 已开始落地：

- 已新增 `apps/runtime/src-tauri/src/commands/im_host/`
- `contract.rs` 已承接 IM lifecycle / delivery contract
- `lifecycle.rs` 已承接 session lifecycle dispatch 组装逻辑
- `chunk_planner.rs` 已承接通用文本分片逻辑
- `delivery_trace.rs` 已承接通用 reply delivery trace 模型
- `target_resolver.rs` 已开始承接 direct/thread/reply-to 的统一目标解析规则
- `runtime_registry.rs` 已开始承接 runtime stdin 写入、request waiter、失败收敛等宿主基础设施
- `runtime_observability.rs` 已开始承接 recent logs / reply lifecycle 的统一观测辅助逻辑
- `runtime_commands.rs` 已开始承接 runtime command payload 的统一命令信封与 builder
- `runtime_events.rs` 已开始承接 runtime event 名称识别与 typed parsing 的统一解析入口
- `inbound_bridge.rs` 已开始承接通用 IM 入站到宿主 dedup / routing / employee session bridge 的统一桥接语义
- `runtime_status.rs` 已开始承接 status/log/fatal 等 runtime status 合并逻辑的统一实现
- `runtime_waiters.rs` 已开始承接 requestId -> waiter result/error/fail_all 的统一投递语义
- `runtime_router.rs` 已开始承接 runtime stdout event 的统一分流骨架
- `runtime_adapter.rs` 已开始承接 runtime stdout handler 的 adapter contract 与统一 dispatcher
- `sidecar_channel.rs` 已开始承接 sidecar channel 的实例 ID、health 归一化与 send-message payload 语义，给非 stdout 型 IM 通道提供同一宿主桥接层
- `startup_restore.rs` 已开始承接通用 IM channel 启动恢复入口，把 plugin runtime restore 与 sidecar connector restore 收到同一宿主层
- `startup_restore.rs` 已进一步收敛为按 channel registry 执行的恢复编排层，飞书与企业微信都通过显式 restore kind 注册接入，不再继续堆叠特判分支
- `openclaw_plugins/feishu_runtime_adapter.rs` 已开始承接 Feishu 专属 stdout handler wiring，`runtime_service.rs` 继续收敛为 runtime 宿主服务
- `openclaw_plugins/wecom_runtime_adapter.rs` 已新增最薄 adapter 壳，用于验证第二个 IM 通道也能复用同一 stdout adapter contract
- Feishu `outbound_service` 已开始直接复用 `im_host` 平台层，而不再只依赖 `openclaw_plugins/im_host_contract.rs`
- Feishu runtime inbound dispatch 也开始复用 `im_host` 的 target resolver，而不再在插件 runtime 内散写一套 direct/chat_id/thread_id 解析
- Feishu runtime outbound command / waiter 管理也开始复用 `im_host` 的 runtime registry 骨架
- Feishu runtime reply lifecycle merge / trimming 也开始复用 `im_host` 的 runtime observability helper
- Feishu runtime command payload 组装也开始复用 `im_host` 的 runtime commands 层，不再保留独立私有 payload struct
- Feishu runtime `send_result / command_error` 等事件解析也开始复用 `im_host` 的 runtime events 层
- Feishu runtime status/log/fatal 合并也开始复用 `im_host` 的 runtime status 层
- Feishu runtime `send_result / command_error` 到 pending waiter 的投递也开始复用 `im_host` 的 runtime waiters 层
- Feishu runtime stdout event 分流也开始复用 `im_host` 的 runtime router 骨架
- Feishu runtime stdout 主循环已开始直接使用 `im_host` 的 dispatcher helper 做 handler wiring，而不只是使用 route 枚举
- Feishu runtime 已开始通过 `ImRuntimeStdoutAdapter` 挂接 handler，stdout 主循环收敛为“decode + route + dispatch”
- Feishu 专属 stdout adapter 已独立成模块，WorkClaw 结构上开始出现“平台宿主 + 渠道 adapter” 的清晰边界
- 企业微信已补上最薄 stdout adapter 骨架，并开始把现有 sidecar channel health / send-message 也挂到 `im_host` 的宿主桥上，证明 `im_host + adapter` 结构不是飞书特例，也不强依赖 stdout runtime
- `channel_connectors` 已新增同步入口，sidecar replay 出来的 WeCom 等 connector 事件可以开始通过统一 `inbound_bridge` 进入宿主，而不再只是停留在 replay/ack 工具面
- `channel_connectors` 已新增后台 monitor 状态机，WeCom connector 启动后可以开始持续 replay/sync/ack，而不再依赖人工反复点击同步
- 桌面启动阶段已开始通过统一 `restore_im_channels(...)` 恢复 IM 能力，不再只对 Feishu 单独硬编码恢复；Feishu runtime restore 与 WeCom connector/monitor restore 开始进入同一平台入口
- 前端设置页已开始暴露 connector monitor 摘要，后台自动恢复与持续同步开始具备可观测性，而不只是后端静默运行
- 前端设置页已新增通用 `channel registry` 总览层，飞书的 OpenClaw 插件宿主与企业微信的 connector 宿主开始以同一套 channel 状态模型展示，而不再把 WeCom 状态硬塞进 Feishu 专属控制器
- Tauri 侧已新增统一 `list_im_channel_registry` 宿主快照入口，前端开始从宿主层一次性读取 Feishu plugin host/runtime 与 WeCom connector/monitor/diagnostics，而不再自行 fan-out 拼装多条状态查询
- `openclaw_gateway` 的通用 IM 入站也开始复用 `inbound_bridge`，逐步收敛到“统一宿主桥 + 渠道入口差异化”的结构
- Feishu `pairing_request / dispatch_request` 已拆成独立 adapter handler，stdout 主入口开始收敛为“router + handler wiring”
- WeCom 侧已补上 host-level lifecycle regression 证据：`ask_user_requested / approval_requested` 进入等待时会先停止 processing，再通过统一 `im_host` contract 发出等待态；`ask_user_answered / approval_resolved / resumed` 也能继续经由统一宿主路由到 WeCom host，不再只在 Feishu 上可验证
- 前端设置页也已补上 WeCom 宿主的统一启停命令证明：和 Feishu 一样通过 `set_im_channel_host_running` 走同一条 channel host control 命令，而不是保留 WeCom 专属开关路径

这意味着 WorkClaw 已经从“仅在 Feishu 内部对齐 OpenClaw”，进入“开始把对齐结果抽成平台能力”的阶段。
