## WorkClaw Windows Release

- Highlights in `v0.4.0`:
  - 中文:
    - 全新桌面端窗口样式，采用自定义标题栏与更轻量的标签页，让 WorkClaw 更接近原生桌面工作台体验。
    - 重构聊天界面层级，统一思考过程、执行记录与最终结果的展示方式，减少视觉噪音并提升阅读稳定性。
    - 会话输入区现在会直接显示当前工作目录与所用模型，发送前的上下文确认更清晰。
    - 开始任务首页新增附件与工作目录支持，启动新任务时可以更自然地带入本地上下文。
    - 统一首页、聊天页、专家技能、智能体员工和设置页的配色与表面层级，整体视觉更协调。
    - 改进聊天滚动与工具结果渲染，减少流式输出过程中打断阅读或跳动的情况。
  - English:
    - Introduces a custom desktop window chrome with a lighter tab strip, making WorkClaw feel more like a native desktop workspace.
    - Reworks the chat layout to better organize reasoning, execution history, and final results with less visual noise and more stable reading flow.
    - The session composer now shows the active workspace and model directly in the input area, making send-time context clearer.
    - The Start Task landing flow now supports attachments and workspace selection, making it easier to launch tasks with local context.
    - Unifies the visual system across landing, chat, experts, employees, and settings for a more consistent desktop experience.
    - Improves chat scroll behavior and tool-result rendering to reduce disruptive jumps during streaming output.

- Recommended download: `*-setup.exe` for direct install.
- Enterprise deployment: `*.msi` for IT-managed installation and manual upgrades.

## Installation Guide

1. Most users should install the `setup.exe` package.
2. Enterprise or managed devices can use the `.msi` package.

## Verification Checklist

- Installer branding and Chinese setup wizard verified.
- Release tag matches desktop app version.
