# OpenClaw IM Host：附录 B - 风险与验证

本文档列出三阶段改造中的主要风险，以及每阶段应重点关注的验证方向。

## 主要风险

### 1. 行为兼容风险

如果 WorkClaw 只做接口对齐、不做生命周期对齐，最终行为仍会与 OpenClaw 官方插件不同步，后续 vendor sync 成本仍然很高。

### 2. Partial delivery 风险

即使去掉前端截断，如果宿主仍然不能表示 `failed_partial`，就仍然会出现“用户看见了前半段，但系统认为整体成功”的盲区。

### 3. Target mapping 风险

direct chat、thread、reply-to、pairing chat_id 等规则若继续分散，飞书与企业微信后续都可能出现消息回到错误目标的问题。

### 4. 测试迁移风险

前端 Feishu fallback 相关测试删除后，如果没有及时补上后端 host/lifecycle 级测试，回归会出现盲区。

### 5. 上游同步风险

如果不明确“哪些行为由 upstream 拥有、哪些行为由 adapter 拥有”，WorkClaw 后续仍会慢慢分叉。

## 第一阶段验证重点

- Feishu 最终答复不再由前端发起
- 长文本 reply 通过 chunk planner 保证完整性
- delivery trace 能区分 completed / failed / failed_partial
- 前端不再出现 Feishu `slice(0, 1800)` 路径

## 第二阶段验证重点

- official lifecycle 的 completion 顺序是否正确
- `waitForIdle` 与 final completion 是否真正对齐
- processing reaction / typing 是否与官方插件一致
- ask_user / approval 是否能在 IM 中完整闭环

## 第三阶段验证重点

- 通用 host contract 是否可同时支撑飞书与企业微信
- runtime registry 是否能统一管理多渠道状态
- target model 是否足以表达各渠道差异
- vendor sync 规则是否可执行

## 建议的回归测试主题

- long reply chunk integrity
- lifecycle order
- processing state start-stop
- ask_user bridge
- approval bridge
- partial delivery trace
- direct / group / thread target consistency

## 验证证据要求

- 关键 completion 行为必须有测试或 trace 证明
- 不能仅凭日志文本声称“已兼容”
- 任何修复“半截回复”问题的改动都应证明：
  - 文本没有被裁切
  - chunk 全部送达
  - 完成态在 idle barrier 之后才标记

## 截至 2026-04-19 的阶段性证据

本轮 Phase 1 收尾时，应以“前端退场、后端接管、reply completion 可观测”为最低证据门槛，而不是只凭代码结构推断。

### 已确认的证据主题

- 前端 Feishu IM bridge 测试应证明：Feishu follow-up 继续通过 `send_message` / `answer_user_question` 进入宿主，不再由 UI 层调用 `send_feishu_text_message` 发送最终答复。
- Rust fast path 应覆盖：reply plan 执行、多 chunk、partial failure、runtime reply lifecycle merge 与 completion projection。
- 设置页测试应覆盖：`latest_reply_completion` 的状态投影、next-step guidance，以及“员工关联入口 / 飞书高级配置”的快捷跳转。
- interactive lifecycle 还应覆盖两类证据：
  - 进入等待时，宿主先停止 processing，再发 `ask_user_requested / approval_requested`
  - 恢复执行时，`ask_user_answered / approval_resolved / resumed` 能继续路由到注册宿主，并在前端展示为“已恢复处理中”
- 企业微信还应额外证明：即使不走 Feishu plugin runtime，connector host 也能复用同一 `im_host` lifecycle contract 与同一宿主启停命令入口，而不是退回私有桥接路径
- 企业微信的最终答复也应证明：统一 `im_host` reply dispatch 入口能直接把 final answer 路由到 WeCom host，而不是只验证 ask_user / approval 这类中间态

### 当前仍需持续盯防的风险

- 即使 Phase 1 已去掉前端 fallback，也不能把“宿主能发出最终回复”等同于“official lifecycle 已完全对齐”；第二阶段仍需用 completion order 测试证明 idle barrier 后才算完成。
- 由于 contract 与 trace 能力已上提到 `im_host/*`，后续改动若只修 Feishu 分支、不补通用层回归，容易重新引入平台与 adapter 漂移。
- `ask_user` / `approval` 目前已具备 reply completion 投影与部分 lifecycle 事件，但仍需继续验证 IM 中断、恢复与 completion 的一致性。
- 当前 Windows 本机环境对部分 `runtime` Rust 单测二进制仍存在 `STATUS_ENTRYPOINT_NOT_FOUND` 风险；因此新增宿主回归需要同时保留 compile-only 证据和 fast-path 证据，避免误把环境问题当成实现失败。

### 2026-04-19 收尾验证记录

- `pnpm test:rust-fast`
  - 结果：PASS
  - 覆盖：Rust fast path 回归，确认与本轮 IM host 收口直接相关的共享 Rust crate 快速回归未被文档冻结动作破坏。
- `pnpm verify:openclaw-im-host:phase3 --compile-only`
  - 结果：PASS
  - 覆盖：以仓库脚本形式复核当前机器可诚实完成的 Phase 3 验证集合，包含：
    - `src/components/__tests__/SettingsView.wecom-connector.test.tsx`
    - `cargo check -p runtime`
    - `pnpm test:rust-fast`
  - 说明：该模式刻意跳过本机仍受 `STATUS_ENTRYPOINT_NOT_FOUND` 影响的 `runtime` libtest 执行步骤，用于给当前环境提供稳定、可复跑的 compile-level 验证入口。
- `pnpm --dir apps/runtime test -- App.im-feishu-bridge.test.tsx`
  - 结果：PASS（17 tests）
  - 覆盖：确认 Feishu follow-up 继续通过宿主侧 `send_message` / `answer_user_question` 路径进入 runtime，UI 层不再承担 `send_feishu_text_message` 最终答复发送责任。
- `pnpm --dir apps/runtime test -- src/components/__tests__/SettingsView.feishu.test.tsx`
  - 结果：PASS（当前已扩展到 6 tests）
  - 覆盖：确认 `latest_reply_completion` 投影、next-step guidance、以及“去员工关联入口 / 打开飞书高级配置”快捷入口行为正常，并新增覆盖 `phase=resumed + state=running` 时显示“已恢复处理中”。
- `pnpm --dir apps/runtime test -- src/components/__tests__/SettingsView.wecom-connector.test.tsx`
  - 结果：PASS（47 tests）
  - 覆盖：确认 WeCom channel registry / diagnostics 正常，并新增覆盖 WeCom 宿主详情通过统一 `set_im_channel_host_running` 执行“启动宿主”；同时修复一条已过时的 Feishu 宿主说明文案断言，保证整份统一渠道设置页测试重新全绿。
- `pnpm --dir apps/runtime exec vitest run ./plugin-host/src/runtime.test.ts --passWithNoTests`
  - 结果：PASS（13 tests）
  - 覆盖：确认 `wait_for_idle -> idle_reached -> fully_complete -> dispatch_idle` 的 barrier 顺序成立，且 `dispatch_idle` 不会被重复发射。
- `cargo check -p runtime`
  - 结果：PASS
  - 覆盖：确认 interactive lifecycle hook、恢复态宿主路由测试辅助、以及 runtime completion projection 调整在 `runtime` crate 级别可编译通过。
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml im_host::interactive_dispatch -- --nocapture`
  - 结果：编译通过，但执行受本机 Windows 环境阻塞
  - 覆盖：新增的 WeCom interactive lifecycle hook 与等待态顺序回归已编入 `runtime_lib` test binary；执行阶段仍落在已知 `STATUS_ENTRYPOINT_NOT_FOUND` 环境问题上。
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml im_host::lifecycle -- --nocapture`
  - 结果：编译通过，但执行受本机 Windows 环境阻塞
  - 覆盖：新增的 WeCom 恢复态 lifecycle regression 与 host-level final reply dispatch regression 已编入 `runtime_lib` test binary；执行阶段仍落在已知 `STATUS_ENTRYPOINT_NOT_FOUND` 环境问题上。

### 本轮验证结论

- 可以把“Phase 1 已建立后端接管 reply orchestration 的结构基线”作为已验证结论记录。
- 目前已经额外获得两类新证据：
  - completion 投影只在 `dispatch_idle` 后才对外显示为完成
  - `ask_user / approval` 的进入等待态与恢复态都已具备更明确的宿主侧可观测性
- 如果本轮新增 WeCom lifecycle 与统一启停命令验证通过，则第三阶段关于“不是 Feishu 特例”的证据链会进一步完整：
  - 等待态顺序与恢复态路由都可在 WeCom 上直接回归
  - WeCom 宿主启停也走统一 channel host command，而不是保留特判入口
- 本轮实际已拿到的新增证据是：
  - WeCom 设置页统一宿主视图重新全绿，并新增验证“启动宿主”同样走统一 channel host command
  - WeCom 的等待态顺序、恢复态路由、以及 final reply dispatch 回归都已完成代码落地并通过 `cargo check -p runtime` 编译校验
- 仍未在本轮声称“official lifecycle 已完全对齐”；`waitForIdle -> markFullyComplete -> markDispatchIdle` 的最终 completion order 仍需在第二阶段继续用更窄、更强的 lifecycle 测试证明。
