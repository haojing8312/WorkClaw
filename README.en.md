# WorkClaw

[简体中文](README.md) | [English](README.en.md)

> Chinese product name: 卧龙AI

<p align="center">
  <img src="docs/workclaw_logo_w.png" alt="WorkClaw Logo" width="140" />
</p>

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18-blue.svg)](https://reactjs.org/)

**Help Everyone Quickly Build Their Own AI Employee Team**

WorkClaw is a beginner-friendly OpenClaw desktop agent distribution that removes command-line and config-file friction. Through conversational interaction, users can install and configure the system, create skills, encrypt/package skills, discover skills across the web, and direct AI teams from mobile via Feishu and other IM channels.

⭐ If you believe AI employee teams should be accessible to everyone, please Star this repository.

## Quick Links

- Getting started: [Getting Started](#getting-started)
- Documentation: [docs/](docs/)
- Operations Manual: [Feishu Wiki](https://my.feishu.cn/wiki/ElrEwHGi7ia78HkKcYXcsVYnnfe)
- Releases: [Releases](https://github.com/haojing8312/WorkClaw/releases)
- Roadmap: [Roadmap](#roadmap)
- Contributing & support: [CONTRIBUTING.md](CONTRIBUTING.md) · [SUPPORT.md](SUPPORT.md)

## Project Status

- Stage: `Active Development (MVP)`
- Primary branch: `main`
- Maintenance mode: ongoing feature iteration + stabilization
- Detailed plans and execution logs: [docs/plans/](docs/plans/)

> **🧪 AI-Powered Development Experiment**
> This project is an experimental demonstration of **100% AI-driven development** - the entire codebase is designed and implemented by AI (Claude Code, GPT-5.3-Codex) without manual code inspection by the developer. This serves as a real-world test of AI's capability to build production-grade software autonomously.

## What is WorkClaw?

WorkClaw's core mission is to make AI employee teams usable by everyone, not just technical experts:
- **For non-technical users**: no command line, no manual config editing; complete setup and usage through chat-like interaction
- **For skill creators**: create, test, encrypt, package, and distribute skills through user-friendly agent workflows
- **For software companies (OEM)**: build and monetize B2B offerings on top of the open-source base
- **For individual users**: easily install, configure, and run a personal AI employee team

WorkClaw also benchmarks against Claude Cowork-style desktop agent experiences with a focus on local control, mobile command capability through IM, and team collaboration.

## Core Product Highlights

- **Start tasks in one sentence**: Use the landing page to start local automation and coding tasks quickly.
- **Agent + tools in one chat loop**: The assistant can read/write files, run commands, and show tool traces while responding.
- **Employee-scoped long-term memory**: Memory is isolated by `employee_id + skill_id`, so each employee agent keeps its own context over time.
- **Default team templates**: On first launch, WorkClaw seeds a built-in "Three Departments and Six Ministries" team that users can run directly or clone into a custom team.
- **Expert Skills workflow**: Create reusable local skills with guided input and real-time `SKILL.md` preview.
- **Built-in packaging flow**: Package skills from the app for secure sharing and distribution.
- **Unified settings control**: Manage models, provider routing, search providers, MCP servers, and runtime options.
- **Default language + immersive translation**: After choosing a default language, English content in Skill Library / Find Skills / chat install candidates can be translated automatically for display.

## Comparison with Commercial Desktop Agents

In early 2026, major companies worldwide have launched desktop AI agent products. Here's a comparison between mainstream commercial products and WorkClaw:

| Product | Vendor | Core Positioning | Key Highlights | Pricing |
|---------|--------|------------------|----------------|---------|
| QClaw (Tencent Security) | Tencent | Personal "Lobster" one-click package | 3-min install + QR bind, WeChat/QQ integration | Free (premium skill packs paid) |
| WorkBuddy (Tencent Cloud) | Tencent Cloud | Enterprise AI desktop assistant | No deployment, 1-min WeCom binding | Free for individuals, per-seat for enterprise |
| Yuanqi AI Bot | Cheetah Mobile (Fu Sheng team) | Domestic OpenClaw alternative | One-click install, free tier only for trial | Paid membership required |
| MiniMax Agent Desktop | MiniMax | AI-native workspace | One-click install, auto-config, built-in expert agents | Free + paid membership |
| ClawX | Valuecell Team | OpenClaw visual client | GUI, zero CLI, 50+ built-in skills | Free basic, $99/year pro |
| LobsterAI (Youdao Lobster) | NetEase Youdao | Chinese-localized OpenClaw | Full Chinese UI, Feishu/DingTalk integration | Free + paid membership |
| MonsterClaw | Independent team | Lightweight OpenClaw wrapper | Minimal install, auto-config environment | Free basic, premium paid |
| **WorkClaw (卧龙AI)** | **Open Source** | **OpenClaw Desktop Agent Distribution** | **Open source, local-only data, no cloud, encrypted packaging for sales** | **Completely free (Apache 2.0)** |

### WorkClaw's Unique Advantages

As a **fully open-source** desktop AI agent, WorkClaw offers key differences from commercial products:

#### 1. Complete Local Data, No Cloud Servers
- All chat logs, employee memories, and skill data stored locally on user devices
- Commercial products typically upload data to cloud, posing privacy risks
- Safe for sensitive industries (finance, healthcare, government)

#### 2. Open Source, Auditable Code
- Source code is completely transparent; you can verify security and privacy
- Commercial products are closed-source; no way to verify data handling
- Enterprises can compile and deploy themselves for full control

#### 3. Encrypted Skill Packaging, Build Your Business
- Built-in **AES-256-GCM** encryption and packaging
- Developers can create skills, encrypt them, and distribute as commercial solutions
- Recipients need username to decrypt and use, protecting IP
- This capability is absent in commercial products

#### 4. Free Choice of LLMs, Pay-Per-Use
- Supports **9 model providers, 15+ large models**
- Including Claude 4.6, GPT-5.3, o1, MiniMax M2.5, GLM-4, DeepSeek V3, Qwen 2.5, Kimi, and more
- Commercial products usually have fixed built-in models with no flexibility
- Pay model providers directly per usage, no middleman markup

#### 5. Completely Free, No Hidden Fees
- Apache 2.0 open source license
- No paywalls, no feature limits
- Free to modify and redistribute

#### 6. Customizable and Forkable
- Based on Tauri + React + Rust tech stack
- Full source code, customizable for specific needs
- Ideal for software companies to OEM B2B products

> **Note**: Some commercial products (like Yuanqi AI Bot) claim "local data storage," but their implementation and auditability cannot match fully open-source WorkClaw. WorkClaw's open-source nature means anyone can verify data processing logic.

### Recommended Scenarios

| Scenario | Recommended |
|----------|-------------|
| Need local data for enterprise/personal | ✅ WorkClaw |
| Need encrypted skill packaging for sales | ✅ WorkClaw |
| Need code audit and security verification | ✅ WorkClaw |
| Need completely free, no paywalls | ✅ WorkClaw |
| Light users wanting quick setup | Commercial options |
| Need official commercial support | Commercial options |

## Default Multi-Employee Team

WorkClaw now includes a built-in complex-task team template and automatically instantiates it for new users on first launch:

- **Seeded on first launch**: the app creates the default "Three Departments and Six Ministries" team, its members, and the baseline collaboration rules automatically.
- **Template vs. instance**: the built-in definition stays system-owned, while the instantiated team in the user's workspace is editable and can be cloned into new variants.
- **Observable runtime**: team runs expose phase, review round, waiting owner, step status, and event history instead of a single generated summary.

## Product Screenshots

### 1) Task Landing
![Task Landing](docs/screenshots/app-home.png)

### 2) Expert Skills Hub
![Expert Skills Hub](docs/screenshots/experts-hub.png)

### 3) Skill Packaging
![Skill Packaging](docs/screenshots/skill-packaging.png)

### 4) Settings
![Settings](docs/screenshots/settings-page.png)

### 5) Packaging Flow (GIF)
![Packaging Flow](docs/screenshots/skill-packaging-demo.gif)

## Architecture

WorkClaw is delivered as a single integrated desktop application:

### Business Architecture

[View business architecture diagram (SVG)](docs/diagrams/business-architecture.svg)

The business architecture covers the end-to-end value stream from creator to user across 4 layers: creator value chain, core platform, user value chain, and ecosystem integration.

### Technical Architecture

[View technical architecture diagram (SVG)](docs/diagrams/technical-architecture.svg)

The technical architecture is organized into 6 layers: user interface, application services, Agent runtime, tool capabilities, model integration, and data persistence.

### WorkClaw Application
The integrated environment where users can package, install, and run encrypted Skills:

**Core Agent Capabilities**:
- ✅ **File Operations**: Read, write, edit files with permission control
- ✅ **Code Execution**: Cross-platform Bash/PowerShell command execution
- ✅ **Browser Automation**: Playwright integration for web scraping and automation (via Sidecar)
- ✅ **MCP Integration**: Model Context Protocol server support for extended capabilities
- ✅ **Multi-Agent System**: Sub-Agent task distribution with isolated contexts
- ✅ **Team-template runtime**: First-launch team seeding, cloneable team instances, and phase-driven collaboration across `plan / review / execute / synthesize`
- ✅ **Layered Memory Management**: TodoWrite tracking + long-term isolation by `employee_id + skill_id`
- ✅ **Web Search**: DuckDuckGo integration for real-time information
- ✅ **Permission System**: Multi-layer security validation

**User Features**:
- Install `.skillpack` files via drag-and-drop or file picker
- Clean chat interface with real-time streaming responses
- No-session landing page with capability intro and scenario templates
- Expert Skills hub (`我的技能`) with guided two-column creation workflow
- Skill packaging entry moved into the Expert Skills domain
- Session history with searchable conversation archives
- Multi-model support (Claude 4.6, GPT-4, MiniMax M2.5, GLM-4, DeepSeek)
- Local secure workspace folder configuration
- Two operation modes: default `Standard Mode` only interrupts truly critical actions, with optional `Full Access`
- Default language and immersive translation settings (`translated_only` / `bilingual_inline`)
- Translation failures fall back to source text and never alter install parameters (`slug` / `githubUrl` / `sourceUrl`)
- No command line required

### Creator Workflow
Creators can develop Skills with **Claude Code** or **VS Code**, then package directly inside the WorkClaw app (no separate Studio client).

## Key Features

### Security & Privacy
- **Military-Grade Encryption**: AES-256-GCM with deterministic key derivation from username
- **Secure Workspace**: Configure trusted local folders for file operations
- **Permission Control**: Default `Standard Mode` only confirms truly critical actions such as delete, destructive overwrite, or external submission; users can switch to `Full Access`
- **No Cloud Dependency**: All processing happens locally

### Agent Capabilities
- **ReAct Loop Engine**: Advanced reasoning and action planning
- **Sub-Agent System**: Parallel task execution with isolated contexts
- **Team templates + runtime state**: Seeded teams can be cloned into custom variants, with visible phase, review round, waiting owner, and event history
- **Context Compression**: Smart truncation to stay within token limits
- **Tool Registry**: Dynamic tool registration including MCP servers
- **Long-Term Memory Isolation**: Employee sessions persist by `employee_id + skill_id`, while non-employee sessions keep the legacy path

### Developer Experience
- **Multi-Model Support**: 15+ models across 9 providers
- **Hot Reload**: Real-time Skill updates during development
- **Comprehensive Logging**: Tool call tracing and error diagnostics
- **Cross-Platform**: Windows, macOS, Linux support

## Tech Stack

### App Backend
- **Framework**: Tauri 2.0 (Rust)
- **Database**: SQLite (sqlx)
- **Encryption**: AES-256-GCM (aes-gcm + ring crates)
- **HTTP Client**: reqwest (for LLM APIs)
- **Sidecar**: Node.js 20+ (Playwright, MCP)

### App Frontend
- **UI**: React 18 + TypeScript
- **Components**: shadcn/ui + Tailwind CSS
- **Markdown**: react-markdown + syntax highlighting
- **State**: React hooks (useState, useEffect)

### Shared Packages
- **skillpack-rs**: Encryption, pack/unpack (Rust)
- **model-adapters**: LLM API adapters (future TS package)

## Supported Models

### Latest Cutting-Edge Models (2026)

**Anthropic Claude**:
- Claude 4.6 Sonnet (latest, best reasoning)

**OpenAI**:
- o1 (latest reasoning model)
- GPT-5.3-Codex (latest coding model, 2026)

**Chinese Leading Models**:
- **MiniMax M2.5** (SWE-Bench 80.2%, code generation)
- **GLM-4** (Zhipu AI, strong Chinese comprehension)
- **DeepSeek V3** (math and reasoning)
- **Qwen 2.5** (Alibaba Cloud, multilingual)
- **Moonshot Kimi** (long context)

**Custom Endpoints**: Any OpenAI-compatible API

## Project Structure

```
workclaw/
├── apps/
│   └── runtime/              # WorkClaw desktop application
│       ├── src/              # React frontend
│       ├── src-tauri/        # Rust backend
│       │   ├── src/
│       │   │   ├── agent/    # Agent system (executor, tools, registry)
│       │   │   ├── adapters/ # LLM adapters (Anthropic, OpenAI)
│       │   │   ├── commands/ # Tauri commands (skills, chat, models, mcp, packaging)
│       │   │   └── db.rs     # SQLite schema
│       │   └── tests/        # Integration tests
│       └── sidecar/          # Node.js sidecar (Playwright, MCP)
├── packages/
│   └── skillpack-rs/         # Encryption library (Rust)
├── docs/                     # Documentation
├── reference/                # Open-source project analysis
└── examples/                 # Example Skills
```

## Getting Started

### Prerequisites

- Rust 1.75+
- Node.js 20+
- pnpm

### Windows Source Build and Release

- For contributor prerequisites, local Tauri startup, and GitHub-based Windows release, see [docs/development/windows-contributor-guide.md](docs/development/windows-contributor-guide.md).

### Development

```bash
# Install dependencies
pnpm install

# Run app in dev mode
pnpm app

# Build for production
pnpm build:app

# Run tests
cd apps/runtime/src-tauri
cargo test
```

### Installing a Skill

1. Open Runtime application
2. Click "Install Skill" or drag `.skillpack` file to window
3. Enter username (used for decryption key derivation)
4. Configure API keys if needed
5. Start chatting!

## Roadmap

### Now (Current Focus)
- Complete the desktop Agent core loop: task execution, tool usage, Skill install, and packaging.
- Improve first-time user experience: conversational setup, model onboarding, and key UX guidance.
- Increase reliability: regression coverage on critical paths and stronger runtime observability.

### Next
- Ship distribution capabilities: auto-update, cross-platform installers, and release channels.
- Strengthen creator workflows: templates, visual editing, and publishing flow.
- Expand ecosystem connectivity: IM remote control, marketplace compatibility, and mobile collaboration.

### Later
- Enterprise capabilities: multi-tenancy, SSO and RBAC, allowlisted Skill installation, controlled work directories, centralized auditing, and enterprise-managed token relay with quotas and rate limits.
- Agent evolution: EvoMap / GEP / A2A integration with traceable evolution.
- Open ecosystem: ongoing compatibility with OpenClaw / ClawHub.

Detailed planning and execution logs are maintained in [docs/plans/](docs/plans/).

## Why "WorkClaw"?

**Work**: Focuses on real task execution, delivery, and team collaboration  
**Claw**: Draws from the OpenClaw ecosystem and the "lobster crew" metaphor for controllable AI workers

The Chinese brand name **Wolong AI (卧龙AI)** is meant to convey strategic intelligence held in reserve, aligned with the product's positioning as an AI employee team you can direct.

Think of it as **"putting your AI employee team to work under your command."**

## Inspiration

Similar to how Cursor and Claude Code democratized AI-assisted coding, WorkClaw aims to democratize AI Skill distribution. Package your expertise once, distribute securely to thousands.

## Planning Notes

This README keeps a high-level roadmap only. Detailed technical plans and iteration logs live in [docs/plans/](docs/plans/).

## ⚠️ Security Disclaimer

Before downloading, installing, compiling, configuring, connecting third-party models or services, importing Skills, or running WorkClaw, read the full [WorkClaw Security Disclaimer](docs/legal/security-disclaimer.en.md).

By downloading, installing, copying, deploying, configuring, integrating, or using WorkClaw, you acknowledge that you have read, understood, and accepted the full disclaimer, including the sections covering product capability boundaries, inherent risks, user security responsibilities, third-party dependency risk, the no-warranty statement, and limitation of liability.

If you do not agree to that disclaimer, do not download, install, deploy, or use WorkClaw.

For vulnerability disclosure and security reporting, see [SECURITY.md](SECURITY.md).

## Advanced Technical Docs (Integrators & Maintainers)

The following docs are optional for most end users and mainly target integrators and maintainers:

- Feishu routing integration (CN): [docs/integrations/feishu-routing.md](docs/integrations/feishu-routing.md)
- Employee identity and memory model (`employee_id`) (CN): [docs/architecture/employee-identity-model.md](docs/architecture/employee-identity-model.md)
- OpenClaw upgrade runbook (CN): [docs/maintainers/openclaw-upgrade.md](docs/maintainers/openclaw-upgrade.md)
- Skill installation troubleshooting (CN): [docs/troubleshooting/skill-installation.md](docs/troubleshooting/skill-installation.md)

## License

Apache 2.0 - see [LICENSE](LICENSE)

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Community

- GitHub Issues: Bug reports and feature requests
- Documentation: [docs/](docs/)
- Examples: [examples/](examples/)
- Reference: [reference/](reference/) - Open-source project analysis
- Support channels: [SUPPORT.md](SUPPORT.md)
- Security reporting: [SECURITY.md](SECURITY.md)
- Code of conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)

## Acknowledgements

- Thanks to the [OpenClaw](https://github.com/openclaw/openclaw) open-source ecosystem for the foundational ideas and capabilities that WorkClaw builds on.

---

**Built with Tauri, React, and Rust** | Inspired by Claude Code, Gemini CLI, and the open-source Agent community
