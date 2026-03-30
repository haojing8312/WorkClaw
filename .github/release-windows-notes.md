## WorkClaw Windows Release

- Highlights in `v0.5.6`:
  - 中文:
    - 提升了飞书相关技能的执行速度。自然语言请求命中可分发的叶子技能后，会更直接地进入确定性执行路径，减少重复试错和无效绕路。
    - 对齐了更多 OpenClaw 风格的技能运行时行为，改进本地 skill 目录投影与命令分发，让依赖同层 runtime 资源的技能运行更稳定。
    - 增强了模型调用的恢复能力。MiniMax 的部分瞬时网关错误现在会自动重试，减少一次性失败直接中断任务的情况。
    - 修复了失败会话中的内部工具桥接记录污染聊天历史的问题，让 transcript 和导出结果更干净、更接近真实执行过程。
    - 继续改进会话恢复、默认模型路由和设置返回后的模型状态同步，减少模型配置刚保存后仍未立即生效的问题。
  - English:
    - Improved execution speed for Feishu-related skills. When a natural-language request resolves to a dispatchable leaf skill, WorkClaw now enters a more deterministic execution path with fewer retries and detours.
    - Aligned more of the skill runtime with OpenClaw-style behavior, improving local skill directory projection and command dispatch so skills that depend on sibling runtime resources run more reliably.
    - Improved model-call resilience. Certain transient MiniMax gateway failures now enter an automatic retry path instead of failing the task immediately.
    - Fixed transcript pollution from internal bridged tool calls in failed sessions, so chat history and exports stay cleaner and closer to the real execution flow.
    - Continued improving session recovery, default model routing, and model-state refresh after leaving settings so newly saved model configurations take effect more reliably.

- Recommended download: `*-setup.exe` for direct install.
- Enterprise deployment: `*.msi` for IT-managed installation and manual upgrades.

## Installation Guide

1. Most users should install the `setup.exe` package.
2. Enterprise or managed devices can use the `.msi` package.

## Verification Checklist

- Installer branding and Chinese setup wizard verified.
- Release tag matches desktop app version.
