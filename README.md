# WorkClaw

[简体中文](README.md) | [English](README.en.md)

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18-blue.svg)](https://reactjs.org/)

**让所有人快速拥有自己的 AI 员工团队**

WorkClaw 是对新手友好的 OpenClaw 桌面智能体发行版：无需命令行和配置文件，通过对话式交互即可完成安装配置、技能创建、加密打包、全网找技能，并结合飞书等 IM 在移动端指挥 AI 团队。

⭐ 如果你认同“让 AI 员工团队人人可用”的方向，欢迎 Star 本仓库。

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
- **专家技能生产流程**：引导式创建可复用本地技能，并实时预览 `SKILL.md`。
- **内置技能打包闭环**：在应用内完成技能打包，便于安全分发与交付。
- **统一设置中心**：集中管理模型、路由策略、搜索引擎、MCP 服务器与运行参数。

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
- ✅ **内存管理**：TodoWrite 任务跟踪，上下文压缩
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
- **内存持久化**：TodoWrite 跨会话任务跟踪

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

### Windows 自动 Release（GitHub）

已支持 `tag` 自动发布 Windows 安装包到 GitHub Release。

```bash
# 1) 确保版本与 tag 一致（apps/runtime/src-tauri/tauri.conf.json -> version）
# 2) 推送语义化 tag（触发 .github/workflows/release-windows.yml）
git tag v0.1.0
git push origin v0.1.0
```

发布前会执行版本一致性校验：`tag(vX.Y.Z)` 必须与 `tauri.conf.json` 的 `version` 相同。

### 安装技能

1. 打开 WorkClaw 应用
2. 点击"安装技能"或拖动 `.skillpack` 文件到窗口
3. 输入用户名（用于密钥推导）
4. 根据需要配置 API 密钥
5. 开始聊天！

## 路线图

### 里程碑 1：Agent Runtime MVP ✨（当前专注）

**核心 Agent 能力**（80% 完成）：
- [x] ReAct 循环执行器，Tool trait 抽象
- [x] 文件操作：Read、Write、Glob、Grep、Edit
- [x] Bash/PowerShell 执行，跨平台支持
- [x] 子 Agent 系统（Task 工具）用于并行任务分发
- [x] TodoWrite 任务管理和内存
- [x] 上下文压缩（token 预算管理）
- [x] 网页搜索（DuckDuckGo）
- [x] WebFetch 用于 URL 内容获取
- [x] AskUser 用于交互式用户输入
- [x] 工具输出截断（30k 字符限制）
- [x] 权限系统（计划中，多层验证）
- [ ] 本地安全工作区文件夹配置
- [ ] MCP 服务器动态注册 UI（70% - 后端已完成）

**技能系统**：
- [x] 技能 YAML frontmatter 解析
- [x] .skillpack 加密/解密 (Rust)
- [x] 安装、列出、删除技能命令
- [x] 从 `.claude/skills/` 目录动态加载技能
- [x] 基于技能的系统提示词注入
- [ ] 开发期间热重载

**Sidecar 集成**：
- [x] Node.js sidecar 管理器（生命周期控制）
- [x] Hono HTTP 服务器（localhost:8765）
- [ ] Playwright 浏览器自动化（15+ 工具）
- [x] MCP 客户端集成（连接、列出工具、调用）
- [ ] 带归一化坐标的浏览器控制器

**多模型支持**：
- [x] Anthropic Messages API 适配器（Claude 模型）
- [x] OpenAI 兼容适配器（GPT、MiniMax、DeepSeek 等）
- [x] 推理内容过滤（DeepSeek、MiniMax）
- [x] 模型配置 UI（API 密钥、基础 URL、模型名称）
- [x] 9 个提供商预设（Claude、OpenAI、MiniMax、DeepSeek、Qwen、Moonshot、GLM、Yi、自定义）

**用户界面**：
- [x] 带流式消息的聊天视图
- [x] Markdown 渲染，语法高亮
- [x] 工具调用可视化卡片
- [x] 子 Agent 嵌套显示
- [x] 会话历史侧边栏
- [x] 设置视图（模型、MCP 服务器）
- [x] AskUser 交互式输入卡片
- [ ] 文件上传支持
- [ ] 安全工作区配置 UI

### 里程碑 2：分发与更新 🚀

**自动更新**：
- [ ] 应用自动更新机制（Tauri updater）
- [ ] 更新服务器基础设施
- [ ] 版本检查和通知
- [ ] 后台下载和安装

**技能版本控制**：
- [ ] 技能版本系统（semver）
- [ ] 升级/降级能力
- [ ] 依赖解析
- [ ] 破坏性变更检测

**打包与安装程序**：
- [ ] Windows：NSIS 安装程序 + 代码签名
- [ ] macOS：DMG + 公证
- [ ] Linux：AppImage + deb/rpm 包

**分发**：
- [ ] 官方下载服务器
- [ ] 镜像 CDN 设置
- [ ] 更新通道（stable、beta、dev）

### 里程碑 3：生态与企业版 🏢

**创作者能力（已并入 WorkClaw 应用）**：
- [ ] Monaco 编辑器集成
- [ ] 技能结构可视化编辑器
- [ ] 嵌入式测试聊天（Claude Code 集成）
- [ ] 一键打包 UI
- [ ] 模板库
- [ ] 发布工作流

**市场**：
- [ ] 基于 Web 的技能市场
- [ ] 搜索和浏览功能
- [ ] 用户评价和评分
- [ ] 支付集成（Stripe/支付宝）
- [ ] 创作者分析仪表板

**企业功能**（参考企业 Agent 架构）：
- [ ] 用户注册和认证（JWT）
- [ ] 多租户支持（团队工作区）
- [ ] 统一模型配置管理
- [ ] 使用配额和计费
- [ ] 管理员仪表板和分析
- [ ] SSO 集成（LDAP、OAuth）
- [ ] 审计日志和合规性
- [ ] 私有技能仓库
- [ ] 基于角色的访问控制（RBAC）
- [ ] 资源使用监控

### 里程碑 4：Agent 进化与生态集成 🧬

**EvoMap 集成**（Agent 自进化）：
- [ ] GEP（基因组进化协议）支持
- [ ] Gene 和 Capsule 数据结构
- [ ] 六步进化循环（扫描 → 信号 → 意图 → 变异 → 验证 → 固化）
- [ ] A2A（Agent-to-Agent）协议客户端
- [ ] 从全球基因池自动继承能力
- [ ] 本地进化历史和审计日志
- [ ] 70/30 资源分配（修复 vs 探索）

**OpenClaw 生态集成**：
- [ ] ClawHub 技能市场浏览器
- [ ] 从 ClawHub 一键导入技能
- [ ] 技能质量评分和安全扫描
- [ ] 社区技能发现和安装

**IM 远程调用**（即时通讯集成）：
- [ ] 企业微信 / 钉钉机器人适配器
- [ ] 带身份验证的安全命令中继
- [ ] 移动端到桌面端技能执行
- [ ] 任务状态通知和流式结果推送
- [ ] 多用户权限隔离

## 为什么叫"WorkClaw"？

**Work**：强调任务执行、协作交付和真实业务产出  
**Claw**：源自 OpenClaw 生态与“小龙虾团队”意象，代表可指挥、可协作的智能体员工

可以理解为 **"让 AI 员工团队在你的指挥下高效工作"**。

## 灵感来源

正如 Cursor 和 Claude Code 使 AI 辅助编码民主化一样，WorkClaw 旨在使 AI 技能分发民主化。打包一次你的专业知识，安全地分发给成千上万的人。

## 未来集成路线图

**Agent 进化**：
- EvoMap 的 GEP（基因组进化协议）和 A2A 通信
- Agent 能力继承和进化机制

**生态集成**：
- ClawHub 市场集成策略
- 社区技能发现和分发

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

## OpenClaw Feishu 路由（内置）

当前版本已内置 OpenClaw 路由核心（不依赖外部 OpenClaw 运行时），并提供：
- Sidecar vendored 路由引擎（`apps/runtime/sidecar/vendor/openclaw-core/`）
- Rust 路由规则持久化（`im_routing_bindings`）
- 设置页「飞书路由规则向导」可视化配置 + 模拟路由
- 聊天页路由决策卡片（`matched_by` / `session_key` / `agent_id`）

## OpenClaw 员工配置（`employee_id` + 对话向导）

当前版本将员工身份统一为单字段 `employee_id`（员工编号）：
- 前端只暴露员工编号，不再要求普通用户理解 `role_id / openclaw_agent_id`。
- 后端保存时自动镜像：`role_id = employee_id`、`openclaw_agent_id = employee_id`。
- 数据库迁移会回填历史数据：`employee_id` 为空时自动使用 `role_id`。

同时新增「对话配置智能体」流程：
- 在员工页可按问答方式生成并预览 `AGENTS.md / SOUL.md / USER.md`。
- 一键应用后写入员工目录：`<employee_work_dir>/openclaw/<employee_id>/`。

技能安装/导入新增重名保护：
- 若显示名冲突，返回 `DUPLICATE_SKILL_NAME:<name>`，前端提示重命名后重试。

## OpenClaw 升级流程

1. 准备 OpenClaw 上游仓库并设置 `OPENCLAW_UPSTREAM_PATH`。
2. 执行 `node scripts/sync-openclaw-core.mjs`。
3. 核对并更新：
   - `apps/runtime/sidecar/vendor/openclaw-core/UPSTREAM_COMMIT`
   - `apps/runtime/sidecar/vendor/openclaw-core/PATCHES.md`
4. 执行回归验证：
   - `pnpm --dir apps/runtime/sidecar test`
   - `cargo test --test test_openclaw_gateway --test test_openclaw_route_regression -- --nocapture`

## 许可证

Apache 2.0 - 详见 [LICENSE](LICENSE)

## 贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 了解详情。

## 社区

- GitHub Issues：错误报告和功能请求
- 文档：[docs/](docs/)
- 示例：[examples/](examples/)
- 参考：[reference/](reference/) - 开源项目分析

## 作者

- WorkClaw 项目作者（个人开发者）
- 个人主页：https://my.feishu.cn/wiki/O62Pwtb94ikFEJkYHuEcxaWanQb

## 开发说明（补充）

本项目包含 AI 驱动开发实践：核心代码由 AI（Claude Code、GPT-5.3-Codex）参与设计与实现，用于持续验证 AI 构建生产级软件的可行性。

## 致谢

- 感谢 [OpenClaw](https://github.com/openclaw/openclaw) 开源生态提供的重要基础能力与灵感，WorkClaw 在其生态基础上持续面向新手友好和企业落地进行增强。

---

**使用 Tauri、React 和 Rust 构建** | 灵感来自 Claude Code、Gemini CLI 和开源 Agent 社区
