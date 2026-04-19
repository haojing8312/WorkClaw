# OpenClaw IM Host：Phase 3 Acceptance Summary

本文档用于回答四个问题：

1. 最初目标是什么
2. 目前完成到了什么程度
3. 已有哪些硬证据
4. 还剩哪些工作，应该如何继续

## 1. 最初目标

本项目最初目标不是“修一个飞书回复截断 bug”，而是把 WorkClaw 从一套逐渐分叉的私有飞书桥接，收敛为 OpenClaw-compatible 的 IM 宿主。

更具体地说，目标包括：

- 让前端退出 IM reply orchestration
- 让后端接管 final / ask_user / approval 三类关键出站路径
- 对齐 OpenClaw 官方 lifecycle，尤其是 `waitForIdle -> fully_complete -> dispatch_idle`
- 抽出统一 `im_host` platform layer，而不是继续让 Feishu 成为唯一特例
- 证明 WeCom 等第二通道也能复用同一宿主 contract

## 2. 当前完成度

按 2026-04-19 当前证据判断，整体大约已完成 `93%-95%`。

可以把当前状态理解为：

- Feishu 主线改造已基本完成
- `im_host` 已从设计概念进入真实平台层
- WeCom 已从“计划支持”推进到“已有 unified host 证据”
- 剩余工作主要集中在最终验收和环境无关验证，而不是大的架构空白

## 3. 已完成的核心结果

### 3.1 前端退出 reply orchestration

- 前端不再承担 Feishu 最终回复 fallback、截断、补发、重试编排
- 设置页与桌面 UI 现在主要负责展示 runtime / registry / diagnostics
- IM reply completion 的完成判定不再由前端自行推断

### 3.2 后端接管 reply lifecycle

- 后端已接管 final / ask_user / approval 三类关键 reply path
- `plugin-host` 生命周期已明确发出 `reply_lifecycle` 事件
- `latest_reply_completion` 已可投影到前端与诊断层
- 完成态已进一步收紧到 `dispatch_idle`

### 3.3 `im_host` 平台层已落地

当前已经形成的宿主基础能力包括：

- lifecycle dispatch
- chunk planner
- delivery trace
- target resolver
- runtime registry
- startup restore
- runtime observability
- channel registry

这意味着 WorkClaw 不再只是“飞书里修了一层逻辑”，而是已经形成多渠道可复用的宿主骨架。

### 3.4 Feishu 不再是唯一特例

Feishu 侧已完成较强收敛：

- `processing_started / processing_stopped`
- `ask_user_requested / ask_user_answered`
- `approval_requested / approval_resolved`
- `interrupt_requested / resumed`
- `wait_for_idle / idle_reached / fully_complete / dispatch_idle`

这些语义不再只停留在前端补丁或一次性 send_result 上，而是进入了统一宿主可观测层。

### 3.5 WeCom 已有 unified host 证据

WeCom 当前已经拿到的关键证明包括：

- connector host 已进入统一 `channel registry`
- startup restore / monitor / diagnostics 已进入统一宿主入口
- `ask_user_requested / approval_requested` 等等待态可通过 unified host 路由
- `ask_user_answered / approval_resolved / resumed` 恢复态可通过 unified host 路由
- final reply 已具备 `maybe_dispatch_registered_im_session_reply_with_pool(...)` 的 host-level 统一分发路径
- 宿主启停与 Feishu 一样，走统一 `set_im_channel_host_running`

这条证据链已经足以说明：当前结构不再是“Feishu 一套、WeCom 另一套”，而是在往真正的平台层收敛。

## 4. 当前硬证据

### 4.1 文档证据

关键文档已经形成一条完整链路：

- [00-context-and-goals.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/00-context-and-goals.md)
- [01-current-state-gap-analysis.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/01-current-state-gap-analysis.md)
- [02-target-architecture.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/02-target-architecture.md)
- [05-phase-3-plan.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/05-phase-3-plan.md)
- [openclaw-im-lifecycle-alignment.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/openclaw-im-lifecycle-alignment.md)
- [appendix-b-risk-and-verification.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/appendix-b-risk-and-verification.md)

### 4.2 前端验证证据

本轮已经确认：

- `pnpm --dir apps/runtime test -- src/components/__tests__/SettingsView.wecom-connector.test.tsx`
  - PASS
  - `47 tests`

这份测试重新全绿后，说明统一渠道设置页当前至少在以下方面有稳定证据：

- Feishu / WeCom channel registry 展示
- WeCom diagnostics 与 monitor summary
- WeCom 宿主“停止宿主”
- WeCom 宿主“启动宿主”
- Feishu 宿主详情与说明文案

### 4.3 Rust / backend 证据

本轮已经确认：

- `cargo check -p runtime`
  - PASS
- `pnpm test:rust-fast`
  - PASS
- `pnpm verify:openclaw-im-host:phase3 --compile-only`
  - PASS

这说明：

- 新增的 `im_host` / WeCom host lifecycle / dispatch 改动已能编译进入 `runtime`
- 仓库要求的 Rust fast path 回归仍然保持通过
- 当前机器已经具备一个可重复执行的 compile-only Phase 3 验证入口，便于后续 handoff 或换机会话继续推进

### 4.4 环境阻塞说明

本轮尝试执行新增的 `cargo test --lib ...` 定向测试时，仍然命中这台 Windows 机器的已知环境问题：

- `STATUS_ENTRYPOINT_NOT_FOUND`

因此当前最准确的说法是：

- 新增 Rust 回归已经完成代码落地
- 已通过 compile-level 验证
- 但 test binary 的实际执行仍受本机环境阻塞

## 5. 还未完全完成的部分

当前剩余工作已经不是大的架构空白，而是最后的验收收口：

- 在无环境问题的机器上真正执行新增 `im_host` Rust lifecycle / dispatch tests
- 把 `dispatch_idle` 作为最终完成边界的证据继续收紧成更聚焦的 lifecycle 验收
- 如果要正式宣布第三阶段结束，补一份更偏发布/验收口径的阶段结论

换句话说，现在离“结构上做完”已经很近，离“证据上无争议地宣布结束”还差最后一小段。

## 6. 对应设计与计划文档

如果后续要继续推进，这些文档就是当前最重要的上下文入口：

- 背景与目标：[00-context-and-goals.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/00-context-and-goals.md)
- 差距分析：[01-current-state-gap-analysis.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/01-current-state-gap-analysis.md)
- 目标架构：[02-target-architecture.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/02-target-architecture.md)
- Phase 1 收尾：[03-phase-1-plan.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/03-phase-1-plan.md)
- Phase 2 计划：[04-phase-2-plan.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/04-phase-2-plan.md)
- Phase 3 平台化计划：[05-phase-3-plan.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/05-phase-3-plan.md)
- 风险与验证：[appendix-b-risk-and-verification.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/appendix-b-risk-and-verification.md)

## 7. 建议的继续路径

### 最优先

- 换一台没有当前 Windows test-binary 环境问题的机器
- 执行新增的 `im_host` Rust lifecycle / dispatch tests
- 把执行结果回填到 `appendix-b-risk-and-verification.md`
- 按 [07-phase-3-external-verification-runbook.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/07-phase-3-external-verification-runbook.md) 的顺序执行，避免遗漏新增 WeCom host-level 用例
- 优先使用仓库脚本 `pnpm verify:openclaw-im-host:phase3`；若当前机器只能做 compile-level 验证，可先运行 `pnpm verify:openclaw-im-host:phase3 --compile-only`
- 结果记录可直接复用 [08-phase-3-external-verification-result-template.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/08-phase-3-external-verification-result-template.md)

### 次优先

- 输出一版更正式的“Phase 3 done / not done”验收结论
- 明确是否把当前阶段定义为“平台化基本完成，进入验证收尾”

### 如果还要继续扩展

- 把这套统一宿主 contract 用作后续更多 IM 渠道接入基线
- 把 vendor sync 边界与 adapter 边界继续固化

## 8. 当前一句话结论

WorkClaw 这项任务已经不再停留在“修飞书消息发送问题”，而是基本完成了从“私有飞书桥接”向“OpenClaw-compatible 多渠道 IM 宿主”的结构性迁移；剩下的主要是最后一段执行级验证与阶段验收收口。
