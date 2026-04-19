# OpenClaw IM Host：Phase 3 Closeout Checklist

本文档把当前分散在 plan、alignment、appendix、runbook 中的收尾动作压缩成一份可直接执行的清单。

适用场景：

- 想知道距离 Phase 3 正式收口还剩哪些任务
- 想判断当前完成度大约是多少
- 想知道下一步应该先做什么，而不是继续翻多份文档

## 当前完成度

按 2026-04-19 当前证据判断，Phase 3 可以保守记为：

- `95%-97%`

相比前一轮的 `93%-95%`，这次提升的主要原因是：

- Windows 已有可执行的 `pnpm test:im-host-windows-regression`
- `pnpm verify:openclaw-im-host:phase3` 已可在当前 Windows 环境完整通过
- plugin-host 侧已补上更窄的 `dispatch_idle` completion-order runtime fixture 回归

## 仍未完成的任务

### A. 必须完成

这些任务完成后，Phase 3 才适合被标记为“执行级验证已收口”。

1. 在非 Windows 或更完整开发环境上补跑原始 `cargo test --lib ...` 定向用例
2. 将执行结果回填到：
   - `appendix-b-risk-and-verification.md`
   - `08-phase-3-external-verification-result-template.md`
3. 输出一版明确的阶段结论：
   - `Phase 3 complete`
   - 或 `Phase 3 complete with known libtest environment caveat`

### B. 建议完成

这些任务不是阻塞项，但能让后续 handoff、vendor sync、继续扩渠道时更稳。

1. 把 Windows / 非 Windows 双路径验收方式沉淀到一页式对外说明
2. 明确原始 `runtime_lib` libtest 与 Windows 专用回归入口的职责边界
3. 将 Phase 3 的“完成定义”固定为：
   - reply completion 以 `dispatch_idle` 为最终外显完成边界
   - WeCom unified-host 路径不再依赖大型 Windows libtest binary 才能验证

### C. 非本阶段阻塞项

这些事可以继续做，但不应该阻止 Phase 3 收口。

1. 把更多 IM 渠道接入统一宿主
2. 继续扩大 vendor sync 覆盖面
3. 进一步整理通用 IM host 的长期治理文档

## 推荐执行顺序

### 第一步

运行并记录一轮“补充证据”验证：

```bash
pnpm verify:openclaw-im-host:phase3
```

如果是在非 Windows 或 libtest 稳定环境上，再补：

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib maybe_notify_registered_ask_user_routes_wecom_session_via_unified_host -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib maybe_notify_registered_approval_requested_routes_wecom_session_via_unified_host -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib host_lifecycle_emit_routes_answer_and_resume_phases_to_wecom_host -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib host_reply_dispatch_routes_wecom_session_via_unified_host -- --nocapture
```

### 第二步

把结果回填到：

- [appendix-b-risk-and-verification.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/appendix-b-risk-and-verification.md)
- [08-phase-3-external-verification-result-template.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/08-phase-3-external-verification-result-template.md)

### 第三步

给出最终阶段判定，建议只在这两个口径里二选一：

- `Phase 3 complete`
- `Phase 3 complete with known Windows runtime_lib libtest caveat`

结论正文可直接复用：

- [10-phase-3-final-status-draft.md](/D:/code/WorkClaw/docs/architecture/openclaw-im-host/10-phase-3-final-status-draft.md)

## 何时可以宣布 Phase 3 结束

满足以下条件即可：

- `pnpm verify:openclaw-im-host:phase3` 已通过
- Windows 专用 WeCom unified-host 回归已通过
- plugin-host `dispatch_idle` completion-order 窄回归已通过
- WeCom waiting-state / resumed / final reply 三类核心能力已有执行级证据
- 外部验收结果已回填

如果原始 `cargo test --lib ...` 在另一台机器上也补齐通过，那么可以更强地宣布：

- `Phase 3 complete`

如果原始 libtest 暂未补齐，但 Windows 专用回归与统一脚本已经通过，则更准确的结论是：

- `Phase 3 complete with known Windows runtime_lib libtest caveat`

## 当前建议

如果只做一件事，我建议下一步直接做这个：

1. 按 runbook 在可用环境上补齐原始 libtest 或明确记录不补的原因
2. 回填结果模板
3. 输出最终阶段结论

这三步做完，Phase 3 就不再是“还差一点的进行中任务”，而是“已收口、可移交、可继续扩展”的完成阶段。
