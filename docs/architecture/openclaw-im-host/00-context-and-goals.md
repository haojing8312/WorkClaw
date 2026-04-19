# OpenClaw IM Host：背景与目标

本文档定义 WorkClaw 在 IM 集成层的目标方向：不再维持一套逐渐分叉的私有飞书桥接，而是把 WorkClaw 重构为 OpenClaw IM 插件生态的兼容宿主。

## 背景

当前 WorkClaw 已经接入飞书官方 OpenClaw CLI 插件，但运行链路仍然混杂了多套职责：

- 前端 `useImBridgeIntegration` 负责 Feishu 最终答复 fallback、重试、截断与补发
- Tauri `feishu_gateway` 负责入站归一化、部分映射与 outbound 调用
- `plugin-host` 暴露了看似兼容 OpenClaw 的 reply helper，但多处仍是 stub
- 官方插件的 `typing`、`waitForIdle`、`markFullyComplete`、chunk send 等生命周期没有被完整承接

这直接带来了几类问题：

- 飞书最终消息存在“只收到前半段”的风险
- WorkClaw 缺少 OpenClaw 官方链路中的 processing reaction / typing 体验
- 前端私有 fallback 使行为与官方插件生态持续漂移
- 企业微信等后续 IM 接入没有统一宿主模型，容易重复造轮子

## 目标

- 在 IM 集成层尽量对齐 OpenClaw 架构与生命周期
- 让 WorkClaw 成为 OpenClaw IM 插件的标准宿主，而不是再设计一套私有 IM 协议
- 移除前端私有 Feishu fallback、截断、补发与重试编排
- 在后端建立统一的 reply lifecycle、chunk planner、delivery trace 与 runtime registry
- 让飞书、企业微信等后续 IM 都按同一宿主 contract 接入

## 非目标

- 不在本轮重构里重写 WorkClaw 的核心 agent/session/runtime 架构
- 不把 WorkClaw 的审批、员工、工作流业务能力迁移到插件侧
- 不要求一次性把所有 OpenClaw 插件都接完
- 不在文档阶段承诺与上游代码逐行同构；本轮目标是行为和边界对齐

## 关键原则

- IM 插件层尽量保持 OpenClaw 原生行为
- WorkClaw 在 IM 层的角色是宿主与适配器，而不是第二套 bot 框架
- 前端不再承担 IM reply orchestration
- “整轮 reply 何时真正完成”必须由后端和插件生命周期共同确认
- 长文本分块、partial failure、delivery trace 必须成为平台能力，而不是某个渠道的局部补丁

## 成功标准

- 飞书处理态、最终回复、ask_user、approval 等关键行为尽量与 OpenClaw 官方插件一致
- WorkClaw 不再通过前端 `get_messages -> send_feishu_text_message` 回推最终答复
- 长文本在飞书中不会因前端截断而丢失后半段
- `plugin-host` 不再用 stub 假装已经完成官方 reply lifecycle
- 企业微信可以复用同一宿主 contract 设计，而不是单独复制飞书桥接逻辑
