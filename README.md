# WorkClaw

[简体中文](README.md) | [English](README.en.md)

> 中文产品名：卧龙AI

<p align="center">
  <img src="docs/workclaw_logo_w.png" alt="WorkClaw Logo" width="140" />
</p>

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18-blue.svg)](https://reactjs.org/)

**让所有人快速拥有自己的 AI 员工团队**

WorkClaw 是一个面向新手的 OpenClaw（小龙虾）桌面应用，把原本偏折腾的安装和配置过程简化成了可直接上手：无需命令行和配置文件，通过对话式交互即可完成安装配置、技能创建、加密打包、全网找技能，并结合飞书等 IM 在移动端指挥 AI 团队。

⭐ 如果你认同“让 AI 员工团队人人可用”的方向，欢迎 Star 本仓库。

## 快速导航

- 快速开始：[快速开始](#快速开始)
- 文档中心：[docs/](docs/)
- 操作手册：[飞书文档](https://my.feishu.cn/wiki/ElrEwHGi7ia78HkKcYXcsVYnnfe)
- 飞书浏览器配置向导：[docs/integrations/feishu-browser-setup.md](docs/integrations/feishu-browser-setup.md)
  - 包含普通用户版“一键安装浏览器桥接（最后一步需在 Chrome 中确认启用）”说明
- 版本发布：[Releases](https://github.com/haojing8312/WorkClaw/releases)
- 路线图：[路线图](#路线图)
- 贡献与支持：[CONTRIBUTING.md](CONTRIBUTING.md) · [SUPPORT.md](SUPPORT.md)

## 项目状态

- 阶段：`Active Development (MVP)`
- 主分支：`main`
- 维护节奏：持续迭代与稳定性修复并行
- 详细计划与执行记录：[docs/plans/](docs/plans/)

## 什么是 WorkClaw？

WorkClaw 的核心目标是把 AI 员工团队从“技术人才专属”变成“所有人都能用”：
- **面向非技术用户**：不写命令行、不改配置文件，用对话完成安装、配置、使用
- **面向技能创作者**：用对话创建技能、调试技能、加密打包技能并安全分发
- **面向软件公司（OEM）**：可基于开源版快速二开，交付 B 端智能体产品并实现商业化
- **面向个人用户**：低门槛安装后即可拥有自己的 AI 员工团队

同时，WorkClaw 对标 Claude Cowork 的桌面智能体体验，强调“本地可控 + 移动端可指挥 + 团队化协作”。

## 核心产品功能速览

- **一句话开始任务**：通过首页输入任务，快速启动本地自动化和开发协作。
- **对话中直接执行工具**：聊天过程中可读写文件、执行命令，并展示工具调用过程。
- **员工级长期记忆**：按 `employee_id + skill_id` 自动隔离记忆，越用越懂每个员工角色。
- **默认团队模板**：首次启动自动预置“三省六部”复杂任务团队，可直接使用或复制为自定义团队。
- **专家技能生产流程**：引导式创建可复用本地技能，并实时预览 `SKILL.md`。
- **内置技能打包闭环**：在应用内完成技能打包，便于安全分发与交付。
- **统一设置中心**：集中管理模型、路由策略、搜索引擎、MCP 服务器与运行参数。
- **默认语言 + 沉浸式翻译**：设置默认语言后，技能库/找技能/聊天候选中的英文内容可自动翻译展示。

## 与商业桌面智能体对比

2026 年初，国内外各大厂商纷纷推出桌面智能体产品。以下是当前市场主流商业产品与 WorkClaw 的对比：

| 产品名称 | 厂商 | 核心定位 | 开箱即用亮点 | 定价模式 |
|---------|------|---------|------------|---------|
| QClaw (腾讯电脑管家版) | 腾讯 | 个人版"小龙虾"一键启动包 | 3分钟安装+扫码绑定，微信/QQ双端直连 | 基础功能免费 (高级技能包付费) |
| WorkBuddy (腾讯云版) | 腾讯云 | 企业级AI智能体桌面办公助手 | 免部署、下载即用，1分钟绑定企微 | 个人版免费，企业版按账号计费 |
| 元气 AI Bot | 猎豹移动 (傅盛团队) | 国产 OpenClaw 平替 | 一键安装，免费额度极少仅供体验 | 会员付费 |
| MiniMax Agent 桌面版 | MiniMax | AI 原生工作台 | 一键安装、自动配置环境，内置专家Agent | 免费版 + 付费会员 |
| ClawX | Valuecell 团队 | OpenClaw 可视化客户端 | 图形化界面，零命令行门槛，内置50+技能 | 基础版免费，专业版99元/年 |
| LobsterAI (有道龙虾) | 网易有道 | 中文本土化 OpenClaw | 全中文界面，飞书/钉钉一键接入 | 免费版 + 付费会员 |
| MonsterClaw | 独立商业团队 | 轻量级 OpenClaw 封装 | 极致简化安装流程，自动配置环境 | 基础免费，高级付费 |
| **WorkClaw (卧龙AI)** | **开源社区** | **OpenClaw 桌面智能体发行版** | **开源免费、数据本地、无云端依赖、技能可加密打包销售** | **完全免费 (Apache 2.0)** |

### WorkClaw 的独特优势

作为 **完全开源** 的桌面智能体，WorkClaw 与商业产品相比具有以下核心差异：

#### 1. 数据完全本地，无云端服务器
- 所有对话记录、员工记忆、技能数据均存储在用户本地设备
- 商业产品数据通常上传云端，存在隐私泄露风险
- 敏感行业（金融、医疗、政府）可放心使用

#### 2. 代码开源可审计
- 源代码完全透明，可自行审查安全性和隐私政策
- 商业产品闭源，无法验证数据处理方式
- 企业可自行编译部署，完全可控

#### 3. 技能加密打包，形成商业闭环
- 内置 **AES-256-GCM** 加密打包功能
- 开发者可创建技能后加密分发，形成自己的商业解决方案
- 接收者需用用户名解密才能使用，保护知识产权
- 这是商业产品不具备的核心能力

#### 4. 自由选择大模型，按量付费
- 支持 **9 个模型提供商、15+ 大模型**
- 包括 Claude 4.6、GPT-5.3、o1、MiniMax M2.5、GLM-4、DeepSeek V3、Qwen 2.5、Kimi 等
- 商业产品通常内置固定模型，无法自由切换
- 大模型费用直接按量付费给模型厂商，无中间商赚差价

#### 5. 完全免费，无隐藏付费
- Apache 2.0 开源许可证
- 无付费墙、无功能限制
- 可自由修改和再分发

#### 6. 可自定义二开
- 基于 Tauri + React + Rust 技术栈
- 完整源码交付，可根据需求定制
- 适合软件公司 OEM 快速交付 B 端产品

> **注意**：部分商业产品（如元气 AI Bot）声称"数据本地存储"，但其实现方式和可审计性无法与完全开源的 WorkClaw 相比。WorkClaw 的开源本质意味着任何人都可以验证数据处理逻辑。

### 适合场景

| 场景 | 推荐选择 |
|-----|---------|
| 需要数据本地化的企业/个人 | ✅ WorkClaw |
| 需要技能加密打包销售 | ✅ WorkClaw |
| 需要代码审计和安全验证 | ✅ WorkClaw |
| 需要完全免费、无付费墙 | ✅ WorkClaw |
| 追求开箱即用的轻度用户 | 商业产品可选 |
| 需要官方商业支持 | 商业产品可选 |

## 默认多员工团队

WorkClaw 当前内置一套默认复杂任务团队模板，并在新用户首次启动时自动实例化为可编辑的员工团队：

- **首启自动预置**：系统会创建默认“三省六部”团队、成员关系和协作规则，无需用户手工搭建。
- **模板与实例分离**：系统内置的是团队模板，用户实际编辑的是自己的团队实例；预置团队可以复制成新的自定义团队。
- **真实运行态可观测**：团队任务会记录阶段、审议轮次、等待对象、步骤状态和事件流，而不是只返回一段摘要。

## 产品截图

### 1) 任务首页
![任务首页](docs/screenshots/app-home.png)

### 2) 专家技能中心
![专家技能中心](docs/screenshots/experts-hub.png)

### 3) 技能打包
![技能打包](docs/screenshots/skill-packaging.png)

### 4) 设置中心
![设置中心](docs/screenshots/settings-page.png)

### 5) 打包流程（GIF）
![打包流程](docs/screenshots/skill-packaging-demo.gif)

## 架构

WorkClaw 以单一集成桌面应用交付：

### 业务架构

[查看业务架构图（SVG）](docs/diagrams/business-architecture.svg)

业务架构展示从创作者到用户的完整价值流，涵盖创作者价值链、核心平台、用户价值链和生态集成 4 层。

### 技术架构

[查看技术架构图（SVG）](docs/diagrams/technical-architecture.svg)

技术架构分为 6 层：用户界面、应用服务、Agent 运行时、工具能力、模型集成和数据持久化，对应 WorkClaw 的桌面端整体实现。

### WorkClaw 应用
统一的 Agent 执行环境，用户可在同一应用内打包、安装和运行加密技能：

**核心 Agent 能力**：
- ✅ **文件操作**：带权限控制的读取、写入、编辑文件
- ✅ **代码执行**：跨平台 Bash/PowerShell 命令执行
- ✅ **浏览器自动化**：Playwright 集成，用于网页抓取和自动化（通过 Sidecar）
- ✅ **MCP 集成**：模型上下文协议服务器支持，扩展能力
- ✅ **多 Agent 系统**：子 Agent 任务分发，独立上下文隔离
- ✅ **团队模板运行时**：支持首启预置团队模板、团队实例复制，以及按 `plan / review / execute / synthesize` 阶段驱动的协作运行
- ✅ **分层记忆管理**：TodoWrite 任务跟踪 + `employee_id + skill_id` 长期记忆隔离
- ✅ **网页搜索**：DuckDuckGo 集成，获取实时信息
- ✅ **权限系统**：多层安全验证

**用户功能**：
- 通过拖放或文件选择器安装 `.skillpack` 文件
- 简洁的聊天界面，实时流式响应
- 无会话首页入口（能力介绍 + 场景模板填充）
- 专家技能中心（我的技能）与左右分栏引导创建页
- 技能打包入口已并入“专家技能”域
- 会话历史，可搜索的对话存档
- 多模型支持（Claude 4.6、GPT-4、MiniMax M2.5、GLM-4、DeepSeek）
- 本地安全工作区文件夹配置
- 两档操作权限：默认“标准模式”仅拦截极高危动作，可切换到“全自动模式”
- 默认语言与沉浸式翻译设置（支持 `translated_only` / `bilingual_inline`）
- 翻译失败自动回退原文，且不影响安装参数（`slug` / `githubUrl` / `sourceUrl`）
- 无需命令行操作

### 创作者工作流
创作者可使用 **Claude Code** 或 **VS Code** 开发技能，并直接在 WorkClaw 应用内完成打包，无需额外客户端。

## 核心特性

### 安全与隐私
- **军事级加密**：AES-256-GCM，基于用户名的确定性密钥推导
- **安全工作区**：为文件操作配置可信任的本地文件夹
- **权限控制**：默认“标准模式”仅在删除、永久覆盖、外部提交等极高危动作时确认，可手动切换为“全自动模式”
- **无云依赖**：所有处理均在本地进行

### Agent 能力
- **ReAct 循环引擎**：高级推理和行动规划
- **子 Agent 系统**：并行任务执行，独立上下文隔离
- **团队模板与运行态**：预置团队可复制为自定义团队，并显示阶段、审议轮次、等待对象与事件流
- **上下文压缩**：智能截断以保持在 token 限制内
- **工具注册表**：动态工具注册，包括 MCP 服务器
- **长期记忆隔离**：员工会话按 `employee_id + skill_id` 持久化，非员工会话兼容旧路径

### 开发者体验
- **多模型支持**：9 个提供商的 15+ 模型
- **热重载**：开发期间实时技能更新
- **全面日志**：工具调用追踪和错误诊断
- **跨平台**：支持 Windows、macOS、Linux

## 技术栈

### 应用后端
- **框架**：Tauri 2.0 (Rust)
- **数据库**：SQLite (sqlx)
- **加密**：AES-256-GCM (aes-gcm + ring crates)
- **HTTP 客户端**：reqwest（用于 LLM API）
- **Sidecar**：Node.js 20+（Playwright、MCP）

### 应用前端
- **UI**：React 18 + TypeScript
- **组件**：shadcn/ui + Tailwind CSS
- **Markdown**：react-markdown + 语法高亮
- **状态**：React hooks (useState, useEffect)

### 共享包
- **skillpack-rs**：加密、打包/解包 (Rust)
- **model-adapters**：LLM API 适配器（未来 TS 包）

## 支持的模型

### 最新尖端模型（2026）

**Anthropic Claude**：
- Claude 4.6 Sonnet（最新，最佳推理）

**OpenAI**：
- o1（最新推理模型）
- GPT-5.3-Codex（最新编程模型，2026）

**国产领先模型**：
- **MiniMax M2.5**（SWE-Bench 80.2%，代码生成）
- **GLM-4**（智谱 AI，强中文理解）
- **DeepSeek V3**（数学和推理）
- **Qwen 2.5**（阿里云，多语言）
- **Moonshot Kimi**（长上下文）

**自定义端点**：任何 OpenAI 兼容 API

## 项目结构

```
workclaw/
├── apps/
│   └── runtime/              # WorkClaw 桌面应用
│       ├── src/              # React 前端
│       ├── src-tauri/        # Rust 后端
│       │   ├── src/
│       │   │   ├── agent/    # Agent 系统（executor, tools, registry）
│       │   │   ├── adapters/ # LLM 适配器（Anthropic, OpenAI）
│       │   │   ├── commands/ # Tauri 命令（skills, chat, models, mcp, packaging）
│       │   │   └── db.rs     # SQLite schema
│       │   └── tests/        # 集成测试
│       └── sidecar/          # Node.js sidecar（Playwright, MCP）
├── packages/
│   └── skillpack-rs/         # 加密库 (Rust)
├── docs/                     # 文档
├── reference/                # 开源项目分析
└── examples/                 # 示例技能
```

## 快速开始

### 前置要求

- Rust 1.75+
- Node.js 20+
- pnpm

### Windows 源码运行与发布

- 贡献者前置要求、Tauri 本地启动流程、GitHub 自动 Release 见：[docs/development/windows-contributor-guide.md](docs/development/windows-contributor-guide.md)

### Windows 下载说明

- 普通用户推荐下载 `*-setup.exe`，安装后默认走应用内自动更新。
- 企业或 IT 统一部署推荐使用 `*.msi`，便于企业环境分发和手工升级。
- 所有公开安装包都发布在 [Releases](https://github.com/haojing8312/WorkClaw/releases) 页面。

### 开发

```bash
# 安装依赖
pnpm install

# 以开发模式运行应用
pnpm app

# 构建生产版本
pnpm build:app

# 运行测试
cd apps/runtime/src-tauri
cargo test
```

### 安装技能

1. 打开 WorkClaw 应用
2. 点击"安装技能"或拖动 `.skillpack` 文件到窗口
3. 输入用户名（用于密钥推导）
4. 根据需要配置 API 密钥
5. 开始聊天！

## 路线图

### Now（当前重点）
- 完成桌面 Agent 核心闭环：任务执行、工具调用、技能安装与打包。
- 打磨新手体验：对话式配置、模型接入、关键交互引导。
- 提升稳定性：关键链路回归测试与运行时可观测性。

### Next（下一阶段）
- 交付分发能力：自动更新、跨平台安装包、发布通道。
- 强化创作者链路：技能模板、可视化编辑、发布流程。
- 拓展生态连接：IM 远程调用、市场兼容与移动端协作。

### Later（长期方向）
- 企业能力：多租户、SSO / RBAC 权限管控、白名单技能安装、受控工作目录、统一审计、企业级 Token 中转与配额限流。
- Agent 进化：EvoMap / GEP / A2A 集成与可追溯演进。
- 开放生态：持续与 OpenClaw / ClawHub 协同兼容。

详细任务拆解与阶段执行记录见 [docs/plans/](docs/plans/)。

## 为什么叫"WorkClaw"？

**Work**：强调任务执行、协作交付和真实业务产出  
**Claw**：源自 OpenClaw 生态与“小龙虾团队”意象，代表可指挥、可协作的智能体员工

中文名 **卧龙AI** 强调“未出山而能定局”的智能谋划与执行能力，更贴合 AI 员工团队的品牌表达。

可以理解为 **"让 AI 员工团队在你的指挥下高效工作"**。

## 灵感来源

正如 Cursor 和 Claude Code 使 AI 辅助编码民主化一样，WorkClaw 旨在使 AI 技能分发民主化。打包一次你的专业知识，安全地分发给成千上万的人。

## 规划说明

README 仅保留高层路线图，详细技术计划与迭代记录维护在 [docs/plans/](docs/plans/)。

## ⚠️ 安全免责声明

在下载、安装、编译、配置、接入第三方模型或服务、导入技能，或运行 WorkClaw 前，请先阅读完整的 [WorkClaw 安全免责声明](docs/legal/security-disclaimer.zh-CN.md)。

您一旦下载、安装、复制、部署、配置、集成或使用 WorkClaw，即视为已阅读、理解并同意该免责声明中关于产品能力边界、固有风险、用户安全义务、第三方依赖风险、无担保声明及责任限制的全部内容。

如果您不同意该免责声明，请勿下载、安装、部署或使用 WorkClaw。

漏洞披露与安全报告流程请参见 [SECURITY.md](SECURITY.md)。

## 进阶技术文档（面向集成与维护）

以下内容主要面向集成方、二开团队和维护者；普通用户可直接跳过：

- 飞书路由集成说明：[docs/integrations/feishu-routing.md](docs/integrations/feishu-routing.md)
- 员工身份与长期记忆模型（`employee_id`）：[docs/architecture/employee-identity-model.md](docs/architecture/employee-identity-model.md)
- OpenClaw 升级维护手册：[docs/maintainers/openclaw-upgrade.md](docs/maintainers/openclaw-upgrade.md)
- Agent-Reach 外部能力接入：[docs/integrations/agent-reach-external-capabilities.md](docs/integrations/agent-reach-external-capabilities.md)
- 技能安装排错（重名冲突等）：[docs/troubleshooting/skill-installation.md](docs/troubleshooting/skill-installation.md)

## 许可证

Apache 2.0 - 详见 [LICENSE](LICENSE)

## 贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 了解详情。

## 社区

- GitHub Issues：错误报告和功能请求
- 文档：[docs/](docs/)
- 示例：[examples/](examples/)
- 参考：[reference/](reference/) - 开源项目分析
- 支持渠道：[SUPPORT.md](SUPPORT.md)
- 安全报告：[SECURITY.md](SECURITY.md)
- 社区行为准则：[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)

## 作者

- WorkClaw 项目作者（个人开发者）
- 个人主页：https://my.feishu.cn/wiki/O62Pwtb94ikFEJkYHuEcxaWanQb

## 开发说明（补充）

本项目包含 AI 驱动开发实践：核心代码由 AI（Claude Code、GPT-5.3-Codex）参与设计与实现，用于持续验证 AI 构建生产级软件的可行性。

## 致谢

- 感谢 [OpenClaw](https://github.com/openclaw/openclaw) 开源生态提供的重要基础能力与灵感，WorkClaw 在其生态基础上持续面向新手友好和企业落地进行增强。

---

**使用 Tauri、React 和 Rust 构建** | 灵感来自 Claude Code、Gemini CLI 和开源 Agent 社区
