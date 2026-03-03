# WorkClaw PRD
## 产品需求文档 v1.0

**文档状态**：草稿
**创建日期**：2026-02-19
**产品定位**：Skill驱动的AI桌面应用发布平台

---

## 目录

1. [产品概述](#1-产品概述)
2. [目标用户](#2-目标用户)
3. [核心概念定义](#3-核心概念定义)
4. [产品架构](#4-产品架构)
5. [功能需求](#5-功能需求)
6. [技术选型](#6-技术选型)
7. [数据模型](#7-数据模型)
8. [开源策略](#8-开源策略)
9. [里程碑规划](#9-里程碑规划)
10. [非功能性需求](#10-非功能性需求)

---

## 1. 产品概述

### 1.1 一句话定义

WorkClaw 是一个开源的 **AI Skill 打包与桌面应用发布平台**：用户在平台上编写或导入已有 Skill，加密打包后发布成可安装的桌面插件，终端用户通过统一的 Runtime 客户端安装使用。

### 1.2 类比定位

| 类比对象 | 他们做什么 | WorkClaw 的差异 |
|---------|-----------|----------------|
| Dify | 编排 LLM 工作流 → 发布成 Web 应用 | 编写 Skill → 发布成**桌面应用** |
| n8n | 可视化节点编排 → 自动化流程 | 文本 Skill → **AI 对话桌面应用** |
| Claude Code Skill | 提示词工作流，源码完全暴露 | Skill 内容加密，**可商业分发** |

### 1.3 核心价值主张

- **对 Skill 创作者**：我会写 Skill，但不会写代码 → 10 分钟发布一个桌面 AI 应用
- **对终端用户**：下载一个客户端 → 安装 Skill 插件 → 直接使用，无需 API Key、无需命令行
- **对整个生态**：Skill 内容加密保护，创作者知识产权有保障，促进高质量 Skill 繁荣

### 1.4 与 Claude Code Skill 的关系

WorkClaw 不替代 Claude Code Skill 生态，而是其**商业化发布层**：

```
[Claude Code Skill 生态]          [WorkClaw Studio]        [桌面用户]
  创作、调试、验证 Skill    ──→   导入/编写 → 加密打包   ──→  Runtime 安装使用
  .claude/skills/目录下的         .skillpack 分发文件
  完整 Skill 目录
```

**两种创作者路径都支持**：
- **路径 A（已有 Skill）**：将 `.claude/skills/` 下的现有 Skill 目录导入 Studio，直接打包发布
- **路径 B（从零创作）**：在 Studio 内置编辑器中新建 Skill，编写、测试、打包一站完成

---

## 2. 目标用户

### 2.1 Skill 创作者（核心用户）

**画像**：
- 熟悉 Claude Code Skill 开发，会写 Markdown 格式的提示词工作流
- 有垂直领域的专业知识（法律、教育、营销、编程等）
- 不懂桌面应用开发，无法独立打包分发
- 有变现意愿（线下收款、私域销售等）

**核心诉求**：
1. 写完 Skill 后，能快速生成可分发的桌面应用
2. Skill 内容不被客户看到（保护知识产权）
3. 控制谁能用（可以随时撤销授权）

### 2.2 终端用户（Skill 的使用者）

**画像**：
- 非技术用户，不了解 API、Claude Code 等概念
- 购买或获得了某个 Skill 的使用权
- 使用场景：特定垂直领域的 AI 助手（法律咨询、写作助理、代码审查等）

**核心诉求**：
1. 像安装普通软件一样安装和使用 AI 应用
2. 不需要自己配置 API Key 和模型
3. 界面简洁，开箱即用

### 2.3 平台管理员（开源自部署场景）

企业或团队自部署 WorkClaw，管理内部 Skill 资产，供内部员工使用。

---

## 3. 核心概念定义

### 3.1 Skill

**定义**：一段结构化的文本（Markdown 格式），描述一个 AI 智能体的行为逻辑。

**组成**：
```markdown
---
name: 合同审查助手
description: 专业的合同风险识别和条款分析工具
version: 1.0.0
model: claude-3-5-sonnet  # 推荐模型，可被运行时覆盖
---

## 角色定义
你是一位拥有10年经验的合同律师...

## 工作流程
1. 首先识别合同类型...
2. 逐条分析风险条款...

## 输出格式
以结构化报告格式输出...
```

### 3.2 SkillPack

**定义**：一个可安装的 Skill 分发包，包含加密的 Skill 内容 + 元数据 + 图标资源。

**文件格式**：`.skillpack`（本质是加密的 zip 包）

一个 Skill 可能由多个文件组成（主入口 + 模板/示例/资源等），完整目录结构打包：

```
myskill.skillpack（加密 zip）
├── manifest.json            # 元数据（明文，Runtime 读取用）
├── icon.png                 # 应用图标（明文）
├── signature.sig            # 整包签名（防篡改）
└── encrypted/               # 以下全部 AES-256-GCM 加密
    ├── SKILL.md.enc         # 主入口文件（必须）
    ├── templates/           # 子模板文件（可选）
    │   ├── outline.md.enc
    │   └── style-guide.md.enc
    ├── examples/            # 示例文件（可选）
    │   └── sample.md.enc
    └── assets/              # 其他资源（可选）
```

**对应原始 Skill 目录结构**（Claude Code 风格）：
```
my-skill/
├── SKILL.md             # 主入口
├── templates/
│   ├── outline.md
│   └── style-guide.md
├── examples/
│   └── sample.md
└── assets/
```

### 3.3 WorkClaw Studio（创作者端）

创作者用来**导入/编写、测试、打包、发布** Skill 的桌面应用。支持两种工作模式：

- **导入模式**：选择本地已有的 Skill 目录（如 `.claude/skills/my-skill/`），Studio 自动识别结构，预览后一键打包
- **编辑器模式**：从零在 Studio 内新建 Skill，Monaco 编辑器 + 测试台一体化

### 3.4 WorkClaw Runtime（用户端）

终端用户用来**安装、管理、运行** SkillPack 的桌面应用。

### 3.5 模型适配器

Runtime 中负责对接不同 AI 模型的抽象层，支持创作者指定推荐模型，用户可覆盖。

---

## 4. 产品架构

### 4.1 整体架构图

```
  [本地已有 Skill 目录]   [从零新建 Skill]
   .claude/skills/xxx/         │
          │                    │
          └──────────┬─────────┘
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                    WorkClaw 生态系统                          │
│                                                             │
│  ┌───────────────────────┐    ┌──────────────────────────┐  │
│  │   Studio（创作者）     │    │    Runtime（终端用户）     │  │
│  │                       │    │                          │  │
│  │  ┌─────────────────┐  │    │  ┌────────────────────┐  │  │
│  │  │ 导入 Skill 目录  │  │    │  │  Skill 管理中心     │  │  │
│  │  │ (目录选择+预览)  │  │    │  │  (已安装列表)       │  │  │
│  │  └─────────────────┘  │    │  └────────────────────┘  │  │
│  │  ┌─────────────────┐  │.sk │  ┌────────────────────┐  │  │
│  │  │ Skill 编辑器     │  │ill │  │  对话界面           │  │  │
│  │  │ (Monaco)        │  │pac │  │  (Chat UI)         │  │  │
│  │  └─────────────────┘  │k   │  └────────────────────┘  │  │
│  │  ┌─────────────────┐  │──► │  ┌────────────────────┐  │  │
│  │  │ 对话测试台       │  │    │  │  模型配置           │  │  │
│  │  └─────────────────┘  │    │  │  (API Key 管理)     │  │  │
│  │  ┌─────────────────┐  │    │  └────────────────────┘  │  │
│  │  │ 加密打包发布     │  │    └──────────────────────────┘  │
│  │  └─────────────────┘  │                                  │
│  └───────────────────────┘                                  │
│                                                             │
│         ┌──────────────────────────────────────┐           │
│         │            模型适配层                  │           │
│         │  Anthropic API  │  OpenAI 兼容 API    │           │
│         │  (Claude)       │  (MiniMax/DeepSeek/ │           │
│         │                 │   Qwen/Kimi/GPT 等) │           │
│         └──────────────────────────────────────┘           │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 两个独立应用

WorkClaw 由两个独立的桌面应用组成，分别服务不同角色：

| | Studio | Runtime |
|---|---|---|
| **目标用户** | Skill 创作者 | 终端用户 |
| **核心功能** | 编写/测试/打包/发布 | 安装/管理/使用 Skill |
| **安装包大小** | ~50MB | ~30MB |
| **技术栈** | Tauri + React | Tauri + React |
| **开源状态** | 完全开源 | 完全开源 |

---

## 5. 功能需求

### 5.1 Studio 功能（P0 = 必须，P1 = 重要，P2 = 可选）

#### 5.1.0 导入已有 Skill（核心入口）

> 这是现有 Claude Code Skill 开发者最高频的使用场景，优先级与编辑器相同。

| 功能 | 优先级 | 描述 |
|------|--------|------|
| 选择本地目录导入 | P0 | 选择 `.claude/skills/xxx/` 等本地 Skill 目录，自动识别结构 |
| 结构预览与验证 | P0 | 展示识别到的文件树，校验 SKILL.md 是否存在，提示缺少的必要字段 |
| Front Matter 补全向导 | P0 | 若 SKILL.md 缺少 name/description/version 等元数据，引导填写 |
| 文件树编辑 | P1 | 导入后可在 Studio 内对文件进行增删改，不影响本地原始目录 |
| 批量导入 | P1 | 一次性选择整个 `.claude/skills/` 目录，批量导入所有 Skill |

**导入后的完整流程**：
```
选择目录 → 结构识别 → 元数据补全 → 测试台验证效果 → 一键打包 → 生成 .skillpack
```

#### 5.1.1 Skill 编辑器

| 功能 | 优先级 | 描述 |
|------|--------|------|
| Markdown 编辑 | P0 | Monaco Editor，语法高亮，自动补全 |
| Skill 模板库 | P0 | 内置常用 Skill 模板（问答、分析、创作等） |
| Front Matter 配置 | P0 | GUI 表单配置 name/description/model 等元数据 |
| 实时预览 | P1 | 编辑时右侧预览渲染效果 |
| AI 辅助生成 Skill | P2 | 通过对话描述需求，自动生成 Skill 草稿 |

#### 5.1.2 对话测试台

| 功能 | 优先级 | 描述 |
|------|--------|------|
| 内嵌 Chat UI | P0 | 直接在 Studio 中测试当前 Skill 效果 |
| 模型切换 | P0 | 测试时可切换不同模型对比效果 |
| 会话历史 | P1 | 保存测试对话，方便对比调整前后效果 |
| 变量注入测试 | P1 | 模拟不同用户输入场景 |

#### 5.1.3 打包发布

| 功能 | 优先级 | 描述 |
|------|--------|------|
| 一键打包为 .skillpack | P0 | 加密 Skill 内容，生成签名，打包元数据 |
| 加密强度选择 | P1 | 基础加密（Fernet）/ 强加密（AES-256-GCM） |
| 版本管理 | P1 | 自动版本号递增，维护版本历史 |
| 发布到 WorkClaw 市场 | P2 | 未来支持官方市场发布 |

#### 5.1.4 模型配置

| 功能 | 优先级 | 描述 |
|------|--------|------|
| API Key 管理 | P0 | 本地安全存储各平台 API Key |
| 模型列表配置 | P0 | 添加/编辑模型端点、名称、参数 |
| 连通性测试 | P0 | 一键测试 API Key 是否有效 |

---

### 5.2 Runtime 功能

#### 5.2.1 Skill 管理

| 功能 | 优先级 | 描述 |
|------|--------|------|
| 安装 .skillpack | P0 | 拖拽或文件选择安装 SkillPack |
| 已安装 Skill 列表 | P0 | 卡片式展示，含名称、描述、版本、作者 |
| 卸载 Skill | P0 | 删除 Skill 及相关数据 |
| 更新 Skill | P1 | 检测并安装新版本 |

#### 5.2.2 对话界面

| 功能 | 优先级 | 描述 |
|------|--------|------|
| Chat UI | P0 | 简洁的对话界面，支持 Markdown 渲染 |
| 多会话管理 | P0 | 创建/切换/删除会话 |
| 会话历史持久化 | P0 | 本地保存对话记录 |
| 文件上传 | P1 | 支持上传文档作为上下文（PDF/TXT/MD） |
| 导出对话 | P1 | 导出为 Markdown 或 PDF |

#### 5.2.3 模型配置

| 功能 | 优先级 | 描述 |
|------|--------|------|
| API Key 配置 | P0 | 用户填写自己的 API Key |
| 模型选择 | P0 | 为每个 Skill 选择使用的模型（可覆盖 Skill 推荐值） |
| 本地模型支持 | P2 | 配置 Ollama 等本地模型端点（企业版功能）|

---

### 5.3 模型适配层

系统同时支持两种主流 API 格式，覆盖市面上所有主要模型：

#### 格式一：Anthropic Messages API

原生格式，用于 Claude 系列模型。

```
POST https://api.anthropic.com/v1/messages
Authorization: x-api-key: <key>
```

支持模型：
- `claude-3-5-sonnet-20241022`
- `claude-3-5-haiku-20241022`
- `claude-3-opus-20240229`

#### 格式二：OpenAI Chat Completions API（兼容格式）

大量国内外模型均支持此格式，仅需配置不同的 Base URL：

| 模型 | Base URL | 备注 |
|------|---------|------|
| OpenAI GPT 系列 | `https://api.openai.com/v1` | 原生 |
| MiniMax M2.5 | `https://api.minimaxi.com/v1` | SWE-Bench 80.2%，推荐首选 |
| MiniMax M2.5-Lightning | `https://api.minimaxi.com/v1` | 高速版，低延迟场景 |
| DeepSeek | `https://api.deepseek.com/v1` | 性价比高 |
| Qwen / 通义千问 | `https://dashscope.aliyuncs.com/compatible-mode/v1` | 阿里云 |
| Moonshot Kimi | `https://api.moonshot.cn/v1` | 长上下文 |
| 智谱 GLM | `https://open.bigmodel.cn/api/paas/v4` | 国内备选 |
| 自定义端点 | 用户填写 | 支持私有部署 |

> **设计原则**：OpenAI 兼容适配器只需实现一次，通过配置不同 Base URL + API Key 即可接入上述所有模型，无需为每个模型单独开发。

**Google Gemini（P1）**：接口格式独立，需单独适配。

#### 适配器接口设计

```typescript
interface ModelAdapter {
  id: string;
  name: string;
  apiFormat: 'anthropic' | 'openai';  // 两种格式
  baseUrl: string;
  chat(messages: Message[], systemPrompt: string, options: ChatOptions): AsyncGenerator<string>;
  testConnection(): Promise<boolean>;
}
```

#### 用户侧配置示意

Runtime 中模型配置界面：
```
添加模型
┌─────────────────────────────────┐
│ 名称：      MiniMax M2.5        │
│ API 格式：  ● OpenAI兼容  ○ Claude│
│ Base URL：  https://api.minimaxi.com/v1 │
│ 模型名称：  MiniMax-M2.5        │
│ API Key：   ****************    │
│            [测试连接]  [保存]   │
└─────────────────────────────────┘
```

---

### 5.4 安全与加密

#### Skill 加密方案

```
[创作者] 打包时：
  Skill 内容 → AES-256-GCM 加密 → skill.enc
  加密密钥   → 派生自 (创作者ID + Skill指纹 + 时间戳)
  整包       → 创作者私钥签名 → signature.sig

[终端用户] 安装/运行时：
  验证 signature.sig（防篡改）
  Runtime 持有解密能力（密钥不落盘，内存中解密）
  用户无法从文件系统直接读取 Skill 内容
```

**保护强度说明**：
- 防止普通用户直接复制粘贴 Skill 内容：**完全防止**
- 防止技术用户逆向 Runtime 提取 Skill：**增加难度**（通过 Tauri 编译+代码混淆）
- 这是合理的保护级别，与 DRM 行业惯例一致

---

## 6. 技术选型

### 6.1 选型原则

1. **跨平台优先**：Windows 为主，同时支持 macOS 和 Linux
2. **轻量优先**：Runtime 安装包 < 30MB，Studio < 50MB
3. **安全优先**：Skill 内容加密，API Key 安全存储
4. **开发效率**：使用成熟的前端生态，降低贡献者门槛

### 6.2 核心技术栈

#### 6.2.1 桌面应用框架：Tauri 2.0

**选择理由**：
- 安装包极小（~8MB 起），远优于 Electron（~150MB）
- Rust 后端提供强安全性，适合加密/密钥管理
- 支持 Windows / macOS / Linux
- 活跃的开源社区，2024 年已发布 v2.0

**对比 Electron 的优势**：
| 指标 | Tauri | Electron |
|------|-------|---------|
| 安装包大小 | ~10-30MB | ~150-300MB |
| 内存占用 | ~50MB | ~200MB+ |
| 安全性 | Rust，内存安全 | Node.js，相对宽松 |
| 代码保护 | 编译为二进制 | ASAR 可解包 |

#### 6.2.2 前端框架：React 18 + TypeScript

**选择理由**：
- 最大的前端开发者社区，降低贡献门槛
- 丰富的组件生态（Monaco Editor、聊天组件等）
- TypeScript 保证代码质量

#### 6.2.3 UI 组件库：shadcn/ui + Tailwind CSS

**选择理由**：
- 无依赖、可复制粘贴的组件，方便定制
- 轻量，不会显著增加包体积
- 现代设计语言，与 Dify/Linear 等产品风格一致

#### 6.2.4 代码编辑器：Monaco Editor

**选择理由**：
- VS Code 同款编辑器，Skill 创作者熟悉
- 支持 Markdown、YAML 语法高亮和自动补全
- 可扩展自定义 Skill 语法提示

#### 6.2.5 本地数据存储：SQLite（via sqlx）

**选择理由**：
- 单文件数据库，无需额外安装，适合桌面应用
- Rust 侧直接操作，性能好
- 存储 Skill 元数据、会话历史、配置等

#### 6.2.6 API Key 安全存储：系统 Keychain

**实现**：Tauri 的 `keyring` 插件，调用：
- Windows：Windows Credential Manager
- macOS：Keychain
- Linux：libsecret

API Key **不存储在明文配置文件**中。

#### 6.2.7 加密库：Rust `aes-gcm` + `ring`

**选择理由**：
- Rust 标准加密库，经过安全审计
- AES-256-GCM 提供认证加密（防篡改+加密）
- 编译进二进制，无外部依赖

### 6.3 技术栈总览

```
┌─────────────────────────────────────────────────────┐
│                   前端层（React + TS）                 │
│  shadcn/ui  │  Monaco Editor  │  Tailwind CSS        │
├─────────────────────────────────────────────────────┤
│                   Tauri IPC 桥接层                    │
├─────────────────────────────────────────────────────┤
│                   Rust 后端层                         │
│  加密/解密（aes-gcm）  │  HTTP 客户端（reqwest）       │
│  SQLite（sqlx）       │  Keychain（keyring）          │
│  文件操作              │  签名验证（ring）              │
├─────────────────────────────────────────────────────┤
│                   操作系统层                           │
│  Windows  │  macOS  │  Linux                         │
└─────────────────────────────────────────────────────┘
```

### 6.4 开发工具链

| 工具 | 用途 |
|------|------|
| Rust 1.75+ | Tauri 后端开发 |
| Node.js 20+ | 前端构建工具链 |
| pnpm | 包管理（monorepo） |
| Turborepo | Monorepo 构建管理 |
| Vitest | 前端单元测试 |
| cargo test | Rust 单元测试 |
| GitHub Actions | CI/CD，自动构建三平台安装包 |

### 6.5 项目仓库结构

```
workclaw/
├── apps/
│   ├── studio/           # Studio 桌面应用
│   │   ├── src/          # React 前端
│   │   └── src-tauri/    # Rust 后端
│   └── runtime/          # Runtime 桌面应用
│       ├── src/          # React 前端
│       └── src-tauri/    # Rust 后端
├── packages/
│   ├── ui/               # 共享 UI 组件库
│   ├── skill-core/       # Skill 解析/验证逻辑（TS）
│   ├── model-adapters/   # 模型适配器（TS）
│   └── skillpack/        # SkillPack 格式定义和工具（Rust）
├── docs/                 # 文档
└── examples/             # 示例 Skill 文件
```

---

## 7. 数据模型

### 7.1 Skill 元数据（manifest.json）

```json
{
  "id": "uuid-v4",
  "name": "合同审查助手",
  "description": "专业的合同风险识别和条款分析工具",
  "version": "1.0.0",
  "author": "张三",
  "author_id": "creator-uuid",
  "created_at": "2026-02-19T00:00:00Z",
  "updated_at": "2026-02-19T00:00:00Z",
  "recommended_model": "claude-3-5-sonnet-20241022",
  "min_context_length": 8000,
  "tags": ["法律", "合同", "风险分析"],
  "license": "commercial",
  "icon": "icon.png"
}
```

### 7.2 本地数据库表结构

```sql
-- 已安装的 Skill
CREATE TABLE installed_skills (
    id TEXT PRIMARY KEY,
    manifest TEXT NOT NULL,      -- JSON
    installed_at DATETIME,
    last_used_at DATETIME,
    skill_enc BLOB NOT NULL      -- 加密的 Skill 内容
);

-- 会话
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    skill_id TEXT NOT NULL,
    title TEXT,
    created_at DATETIME,
    model_id TEXT NOT NULL
);

-- 消息
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,          -- 'user' | 'assistant'
    content TEXT NOT NULL,
    created_at DATETIME
);

-- 模型配置
CREATE TABLE model_configs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider TEXT NOT NULL,      -- 'anthropic' | 'openai' | 'ollama' | 'custom'
    base_url TEXT NOT NULL,
    model_name TEXT NOT NULL,
    is_default BOOLEAN DEFAULT FALSE
);
```

---

## 8. 开源策略

### 8.1 许可证选择

参考 n8n 和 Dify 的策略，采用**双许可证模式**：

| 版本 | 许可证 | 适用场景 |
|------|--------|---------|
| 社区版 | Apache 2.0 | 个人使用、自部署、非商业 |
| 企业版（未来） | 商业许可 | 云端托管、团队管理、高级功能 |

> **说明**：Studio 和 Runtime 核心功能完全开源，企业版功能（用户管理、云端 Skill 同步、使用分析）作为闭源附加模块。

### 8.2 社区版 vs 企业版功能对比

| 功能 | 社区版 | 企业版（未来） |
|------|--------|--------------|
| Skill 编写和测试 | ✅ | ✅ |
| 打包为 .skillpack | ✅ | ✅ |
| 本地安装和运行 | ✅ | ✅ |
| 多模型支持 | ✅ | ✅ |
| 云端 Skill 分发 | ❌ | ✅ |
| 团队协作 | ❌ | ✅ |
| 用户使用统计 | ❌ | ✅ |
| License 授权控制 | ❌ | ✅ |
| SSO / LDAP | ❌ | ✅ |

### 8.3 项目名称

**建议名称**：`WorkClaw`（暂定）
**GitHub 组织**：`workclaw-dev`（待定）
**文档站**：基于 Docusaurus 构建

---

## 9. 里程碑规划

### Milestone 1：核心可用（MVP）

**目标**：Studio 可以编写和测试 Skill，打包生成 .skillpack；Runtime 可以安装和运行。

**功能范围**：
- [ ] Studio：本地 Skill 目录导入（目录选择 + 结构识别 + 元数据补全向导）
- [ ] Studio：Skill 编辑器（Monaco）+ Front Matter 表单
- [ ] Studio：内嵌对话测试台
- [ ] Studio：一键打包为 .skillpack（AES-256-GCM 加密，支持多文件目录）
- [ ] Runtime：安装 .skillpack（拖拽/文件选择）
- [ ] Runtime：基础 Chat UI（Markdown 渲染 + 流式输出）
- [ ] Runtime：会话历史持久化
- [ ] 模型适配：Anthropic Messages API（Claude 系列）
- [ ] 模型适配：OpenAI Chat Completions 兼容格式（MiniMax/DeepSeek/Qwen/GPT 等，配置 Base URL 即可）
- [ ] 模型配置：API Key 安全存储（系统 Keychain）

### Milestone 2：体验完善

**目标**：打磨细节，达到可公开发布的质量。

**功能范围**：
- [ ] Studio：Skill 模板库（内置 10 个常用模板）
- [ ] Studio：版本管理
- [ ] Runtime：文件上传支持
- [ ] Runtime：多会话管理优化
- [ ] 模型适配：Gemini（独立适配）
- [ ] 自动更新机制
- [ ] Windows 安装包（NSIS）+ macOS DMG

### Milestone 3：生态建设

**目标**：建立 Skill 分发和社区机制。

**功能范围**：
- [ ] 官方 Skill 市场（Web）
- [ ] Studio：发布到市场
- [ ] Runtime：从市场安装
- [ ] AI 辅助生成 Skill（对话引导）
- [ ] Linux AppImage

---

## 10. 非功能性需求

### 10.1 性能

| 指标 | 目标值 |
|------|--------|
| 应用启动时间 | < 2 秒 |
| Skill 安装时间 | < 1 秒 |
| 首条消息响应（流式） | < 1 秒开始输出 |
| 安装包大小（Runtime） | < 30MB |
| 安装包大小（Studio） | < 50MB |
| 内存占用（Runtime 空闲） | < 100MB |

### 10.2 安全性

- API Key 仅存储在系统 Keychain，不落盘明文
- Skill 内容 AES-256-GCM 加密，运行时内存解密
- 网络请求仅发往用户配置的 AI API 端点，无第三方数据收集
- Tauri 默认 CSP，防止 XSS

### 10.3 兼容性

- Windows 10/11（x64）
- macOS 12+（Intel + Apple Silicon）
- Linux（Ubuntu 20.04+，AppImage）

### 10.4 可访问性

- 支持系统深色/浅色主题
- 字体大小可调
- 键盘快捷键支持主要操作

---

## 附录

### A. 竞品分析快照

| 产品 | 发布形式 | 是否开源 | Skill 保护 | 多模型 |
|------|---------|---------|----------|--------|
| Dify | Web 应用 | 是（Apache） | 有 | 是 |
| n8n | Web 应用 | 是（可持续许可） | 有 | 有限 |
| Claude Code Skill | 本地文件 | 是 | 无 | 否 |
| **WorkClaw** | **桌面应用** | **是（Apache）** | **AES-256** | **是** |

### B. 关键技术风险

| 风险 | 概率 | 影响 | 缓解方案 |
|------|------|------|---------|
| Tauri 生态不成熟 | 低 | 中 | 已验证 v2.0 稳定，有大量案例 |
| Skill 加密被逆向 | 中 | 中 | 明确说明保护级别，非 DRM 级别 |
| 模型 API 变更 | 中 | 低 | 适配器模式隔离变更 |
| 包体积超标 | 低 | 低 | Tauri 天然轻量，持续监控 |

### C. 参考项目

- [Tauri 官方文档](https://tauri.app/)
- [shadcn/ui](https://ui.shadcn.com/)
- [Monaco Editor](https://microsoft.github.io/monaco-editor/)
- [Dify 开源仓库](https://github.com/langgenius/dify)
- [n8n 开源仓库](https://github.com/n8n-io/n8n)

---

*文档版本：v1.0 | 最后更新：2026-02-19*
