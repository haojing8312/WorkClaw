# WorkClaw

[简体中文](README.md) | [English](README.en.md)

> 中文产品名：打工虾

<p align="center">
  <img src="docs/workclaw_logo_w.png" alt="WorkClaw Logo" width="140" />
</p>

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18-blue.svg)](https://reactjs.org/)

**让所有人快速拥有自己的 AI 员工团队**

WorkClaw 是对新手友好的 OpenClaw 桌面智能体发行版：无需命令行和配置文件，通过对话式交互即可完成安装配置、技能创建、加密打包、全网找技能，并结合飞书等 IM 在移动端指挥 AI 团队。

⭐ 如果你认同“让 AI 员工团队人人可用”的方向，欢迎 Star 本仓库。

## 快速导航

- 快速开始：[快速开始](#快速开始)
- 文档中心：[docs/](docs/)
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
- **专家技能生产流程**：引导式创建可复用本地技能，并实时预览 `SKILL.md`。
- **内置技能打包闭环**：在应用内完成技能打包，便于安全分发与交付。
- **统一设置中心**：集中管理模型、路由策略、搜索引擎、MCP 服务器与运行参数。
- **默认语言 + 沉浸式翻译**：设置默认语言后，技能库/找技能/聊天候选中的英文内容可自动翻译展示。

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

![业务架构](docs/diagrams/business-architecture.svg)

业务架构展示了从创作者到用户的完整价值流，分为 4 层：
- **创作者价值链**：技能开发 → 打包/加密 → 发布
- **核心平台**：Agent 引擎、安全系统、工具能力、模型集成
- **用户价值链**：个人用户（浏览 → 安装 → 运行）+ 企业用户（团队/RBAC → 统一配置/SSO → Agent 员工）
- **生态集成**：EvoMap 进化、WorkClaw 市场 + ClawHub 兼容、IM 远程调用

### 技术架构

![技术架构](docs/diagrams/technical-architecture.svg)

技术栈分为 6 层：
- **第 1 层 - 用户界面**：React 18 + TypeScript, shadcn/ui + Tailwind, Tauri 2.0 WebView
- **第 2 层 - 应用服务**：Rust 后端, Node.js Sidecar (localhost:8765)
- **第 3 层 - Agent 运行时**：ReAct 引擎、子 Agent 隔离、上下文管理、skillpack-rs 加密
- **第 4 层 - 工具能力**：原生工具（Read/Write/Glob/Grep）、Bash/PowerShell、浏览器自动化、MCP 协议
- **第 5 层 - 模型集成**：Anthropic API、OpenAI 兼容、国产模型（MiniMax、DeepSeek、GLM、Qwen、Moonshot）
- **第 6 层 - 数据持久化**：SQLite、.skillpack 文件、安全工作区文件夹

### WorkClaw 应用
统一的 Agent 执行环境，用户可在同一应用内打包、安装和运行加密技能：

**核心 Agent 能力**：
- ✅ **文件操作**：带权限控制的读取、写入、编辑文件
- ✅ **代码执行**：跨平台 Bash/PowerShell 命令执行
- ✅ **浏览器自动化**：Playwright 集成，用于网页抓取和自动化（通过 Sidecar）
- ✅ **MCP 集成**：模型上下文协议服务器支持，扩展能力
- ✅ **多 Agent 系统**：子 Agent 任务分发，独立上下文隔离
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
- 默认语言与沉浸式翻译设置（支持 `translated_only` / `bilingual_inline`）
- 翻译失败自动回退原文，且不影响安装参数（`slug` / `githubUrl` / `sourceUrl`）
- 无需命令行操作

### 创作者工作流
创作者可使用 **Claude Code** 或 **VS Code** 开发技能，并直接在 WorkClaw 应用内完成打包，无需额外客户端。

## 核心特性

### 安全与隐私
- **军事级加密**：AES-256-GCM，基于用户名的确定性密钥推导
- **安全工作区**：为文件操作配置可信任的本地文件夹
- **权限控制**：敏感操作的多层验证
- **无云依赖**：所有处理均在本地进行

### Agent 能力
- **ReAct 循环引擎**：高级推理和行动规划
- **子 Agent 系统**：并行任务执行，独立上下文隔离
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

### Windows 贡献者前置要求（源码运行）

如果你只是想使用 WorkClaw，请优先下载 Release 安装包。下面这些要求是给 **从源码运行桌面应用** 的贡献者准备的。

- Windows 10 / 11 x64
- Rust stable + `x86_64-pc-windows-msvc`
- Visual Studio 2022 Build Tools（稳定版）
- `Desktop development with C++`
- Windows 10/11 SDK
- WebView2 Runtime

如果本地构建失败，先运行：

```bash
pnpm doctor:windows
```

常见 Windows 本地构建问题请看：

- [docs/troubleshooting/windows-dev-setup.md](docs/troubleshooting/windows-dev-setup.md)

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

### 本地快速启动 Tauri 窗口（稳定流程）

```bash
# 1) 仅首次或依赖变更后执行
pnpm install

# Windows 源码构建失败时，先做本机环境诊断
pnpm doctor:windows

# 2) 若报错 "Port 5174 is already in use"，先定位并结束占用进程
netstat -ano | findstr LISTENING | findstr :5174
taskkill /PID <PID> /F

# 3) 从仓库根目录启动 Tauri 桌面窗口
pnpm app
```

启动成功后可用下面两条命令快速自检：

```bash
# 前端开发服务已启动（应返回 HTTP 200）
curl -I http://localhost:5174

# Tauri 桌面进程已启动（应看到 runtime.exe）
tasklist | findstr /I runtime.exe
```

退出测试（按需）：

```bash
# 先结束 5174 端口监听进程
netstat -ano | findstr LISTENING | findstr :5174
taskkill /PID <PID> /F

# 再结束 runtime.exe 对应 PID（只杀你本次测试启动的 PID）
tasklist | findstr /I runtime.exe
taskkill /PID <RUNTIME_PID> /F
```

### Windows 自动 Release（GitHub）

已支持 `tag` 自动发布 Windows 安装包到 GitHub Release。

```bash
# 1) 确保版本与 tag 一致（apps/runtime/src-tauri/tauri.conf.json -> version）
# 2) 推送语义化 tag（触发 .github/workflows/release-windows.yml）
git tag v0.1.0
git push origin v0.1.0
```

发布前会执行版本一致性校验：`tag(vX.Y.Z)` 必须与 `tauri.conf.json` 的 `version` 相同。

发布产物使用建议：

- `*-setup.exe`：推荐普通用户下载，支持后续桌面端自动更新。
- `*.msi`：适合企业 IT、批量部署和手动升级，不参与应用内自动更新。
- 自动更新链路：已安装 `setup.exe` 版本的桌面端会消费 GitHub Release 中的 `latest.json` 和 `.sig`。

如果你只是想安装并直接使用 WorkClaw，请优先选择 `.exe` 安装包。

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
- 企业能力：多租户、RBAC、审计、SSO、配额计费。
- Agent 进化：EvoMap / GEP / A2A 集成与可追溯演进。
- 开放生态：持续与 OpenClaw / ClawHub 协同兼容。

详细任务拆解与阶段执行记录见 [docs/plans/](docs/plans/)。

## 为什么叫"WorkClaw"？

**Work**：强调任务执行、协作交付和真实业务产出  
**Claw**：源自 OpenClaw 生态与“小龙虾团队”意象，代表可指挥、可协作的智能体员工

可以理解为 **"让 AI 员工团队在你的指挥下高效工作"**。

## 灵感来源

正如 Cursor 和 Claude Code 使 AI 辅助编码民主化一样，WorkClaw 旨在使 AI 技能分发民主化。打包一次你的专业知识，安全地分发给成千上万的人。

## 规划说明

README 仅保留高层路线图，详细技术计划与迭代记录维护在 [docs/plans/](docs/plans/)。

## ⚠️ 安全免责声明

**重要 - 使用前必读**

桌面 Agent 具有强大的能力，包括文件系统访问和命令执行。这带来固有的安全风险：

- **恶意技能**：第三方可能会分发包含有害代码的 `.skillpack` 文件
- **系统访问**：已安装的技能可以读取、修改或删除您计算机上的文件
- **命令执行**：技能可以使用您的用户权限执行任意 Shell 命令
- **数据暴露**：技能可能访问工作区文件夹中的敏感数据

**下载、安装或运行本软件即表示您确认：**
1. 您理解桌面 AI Agent 相关的安全风险
2. 您只会从可信来源安装技能
3. 您会仔细审查和配置工作区权限
4. **开发者对使用本软件或通过本软件安装的任何技能所导致的任何损害、数据丢失或安全漏洞不承担任何责任**

**如果您不同意这些条款，请勿下载、安装或运行本软件。**

安全最佳实践请参见 [SECURITY.md](SECURITY.md)。

## 进阶技术文档（面向集成与维护）

以下内容主要面向集成方、二开团队和维护者；普通用户可直接跳过：

- 飞书路由集成说明：[docs/integrations/feishu-routing.md](docs/integrations/feishu-routing.md)
- 员工身份与长期记忆模型（`employee_id`）：[docs/architecture/employee-identity-model.md](docs/architecture/employee-identity-model.md)
- OpenClaw 升级维护手册：[docs/maintainers/openclaw-upgrade.md](docs/maintainers/openclaw-upgrade.md)
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
