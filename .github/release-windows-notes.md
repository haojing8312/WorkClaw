## WorkClaw Windows Release

- Highlights in `v0.5.3`:
  - 中文:
    - 强化了智能体运行时稳定性，补上会话内运行串行保护、工具循环拦截、转录清洗和上下文压缩治理，长任务与工具链路更稳。
    - 改善了多步任务执行可靠性，子任务现在由运行时托管并持久化，复杂任务更不容易出现状态漂移或中断。
    - 完成聊天页、员工中心和设置页的一轮场景层重构与主干收口，提升维护性，也修复了合并过程中暴露出的细节回归。
    - 继续加固 Windows 桌面打包链路，`MSI` 与 `NSIS` 安装包构建通过，飞书设置与桌面端整体稳定性进一步提升。
  - English:
    - Strengthens agent runtime stability with per-session run serialization, pre-tool loop interception, transcript hygiene, and context compaction governance for more reliable long-running and tool-heavy tasks.
    - Improves multi-step task reliability by moving subtask execution into runtime-owned persistent child sessions, reducing state drift and interrupted execution.
    - Completes a scene-layer refactor and mainline integration pass for Chat, Employee Hub, and Settings, improving maintainability and fixing merge-time regressions found during integration.
    - Further hardens the Windows desktop packaging path, with both `MSI` and `NSIS` installers building successfully and overall Feishu settings plus desktop flow stability improved.

- Recommended download: `*-setup.exe` for direct install.
- Enterprise deployment: `*.msi` for IT-managed installation and manual upgrades.

## Installation Guide

1. Most users should install the `setup.exe` package.
2. Enterprise or managed devices can use the `.msi` package.

## Verification Checklist

- Installer branding and Chinese setup wizard verified.
- Release tag matches desktop app version.
