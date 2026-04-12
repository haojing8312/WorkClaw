## WorkClaw Windows Release

- Release scope: changes from `v0.5.9` to the current `v0.5.10` tag target.

## Highlights

- 中文:
  - 对齐了新一代核心智能体运行时。WorkClaw 现在把本地对话、子会话、员工步骤和团队执行统一到同一套任务引擎与运行链路，减少多智能体流程中的状态分叉。
  - 完善了智能体员工与团队协作体验。员工中心、团队执行、人格与 workspace 适配层已收口到新的核心能力模型，后续扩展会更稳定。
  - 改进了长任务连续性与恢复能力。继续执行、恢复执行、任务回流和会话导出现在能携带更完整的 continuation 上下文，长链路任务更不容易“断片”。
  - 修复了 Windows 下真实评测与桌面构建链路中的一批历史问题，包括 `agent_eval` 启动方式、前端类型漂移和发布打包阻塞。
  - 清理了运行时与品牌生成链路中的冗余 warning 与重复包装层，让本地构建、回归和发布过程更稳定。

- English:
  - Aligned WorkClaw with the new core agent runtime. Local chat, child sessions, employee steps, and team execution now flow through the same task engine and runtime path, reducing state drift in multi-agent workflows.
  - Improved employee and team collaboration flows. The employee hub, team execution, persona handling, and workspace adapter layers now sit on the new core capability model, making future expansion more stable.
  - Strengthened long-running task continuity and recovery. Continue, resume, task return, and session export flows now preserve richer continuation context, which makes longer task chains more reliable.
  - Fixed several long-standing Windows real-eval and desktop build issues, including the `agent_eval` startup path, frontend type drift, and release packaging blockers.
  - Removed redundant runtime wrappers and warning-heavy build noise so local builds, regressions, and release flows are more stable.

## Notable Changes

- Core runtime alignment:
  - Unified local chat, delegated task, employee step, and team execution paths under the task engine.
  - Added close-code style agent catalog, spawn policy, and employee runtime adapter layers.

- Continuity and recovery:
  - Carried continuation state through control-plane, recovery, parent rejoin, and export flows.
  - Hardened OpenAI-compatible tool calling and long-session responsiveness.

- Desktop and release hardening:
  - Fixed Windows-specific real regression startup issues for `agent_eval`.
  - Restored full frontend build compatibility and desktop packaging on `main`.

- Recommended download: `*-setup.exe` for direct install.
- Enterprise deployment: `*.msi` for IT-managed installation and manual upgrades.

## Installation Guide

1. Most users should install the `setup.exe` package.
2. Enterprise or managed devices can use the `.msi` package.

## Verification Checklist

- Core runtime, frontend build, and Windows desktop packaging were verified on the release branch.
- Release version files and release notes validated against the `v0.5.10` tag target.
- Local Windows packaging is re-run as part of this release flow.
- Release tag matches desktop app version.
