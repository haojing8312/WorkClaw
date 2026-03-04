# WorkClaw

[简体中文](README.md) | [English](README.en.md)

<p align="center">
  <img src="apps/runtime/src/assets/branding/workclaw-logo.png" alt="WorkClaw Logo" width="140" />
</p>

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18-blue.svg)](https://reactjs.org/)

**Help Everyone Quickly Build Their Own AI Employee Team**

WorkClaw is a beginner-friendly OpenClaw desktop agent distribution that removes command-line and config-file friction. Through conversational interaction, users can install and configure the system, create skills, encrypt/package skills, discover skills across the web, and direct AI teams from mobile via Feishu and other IM channels.

⭐ If you believe AI employee teams should be accessible to everyone, please Star this repository.

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
- **Expert Skills workflow**: Create reusable local skills with guided input and real-time `SKILL.md` preview.
- **Built-in packaging flow**: Package skills from the app for secure sharing and distribution.
- **Unified settings control**: Manage models, provider routing, search providers, MCP servers, and runtime options.

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

![Business Architecture](docs/diagrams/business-architecture.svg)

The business architecture showcases the complete value stream from creator to user, organized in 4 layers:
- **Creator Value Chain**: Skill development → package/encrypt → publish
- **Core Platform**: Agent engine, security, tool capabilities, model integration
- **User Value Chain**: Personal (browse → install → run) + Enterprise (team/RBAC → unified config/SSO → Agent employees)
- **Ecosystem Integration**: EvoMap evolution, WorkClaw marketplace + ClawHub compatibility, IM remote calling

### Technical Architecture

![Technical Architecture](docs/diagrams/technical-architecture.svg)

The technical stack is organized in 6 layers:
- **Layer 1 - User Interface**: React 18 + TypeScript, shadcn/ui + Tailwind, Tauri 2.0 WebView
- **Layer 2 - Application Services**: Rust Backend, Node.js Sidecar (localhost:8765)
- **Layer 3 - Agent Runtime**: ReAct engine, Sub-Agent isolation, Context management, skillpack-rs encryption
- **Layer 4 - Tool Capabilities**: Native Tools (Read/Write/Glob/Grep), Bash/PowerShell, Browser automation, MCP protocol
- **Layer 5 - Model Integration**: Anthropic API, OpenAI Compatible, Chinese models (MiniMax, DeepSeek, GLM, Qwen, Moonshot)
- **Layer 6 - Data Persistence**: SQLite, .skillpack files, Secure workspace folders

### WorkClaw Application
The integrated environment where users can package, install, and run encrypted Skills:

**Core Agent Capabilities**:
- ✅ **File Operations**: Read, write, edit files with permission control
- ✅ **Code Execution**: Cross-platform Bash/PowerShell command execution
- ✅ **Browser Automation**: Playwright integration for web scraping and automation (via Sidecar)
- ✅ **MCP Integration**: Model Context Protocol server support for extended capabilities
- ✅ **Multi-Agent System**: Sub-Agent task distribution with isolated contexts
- ✅ **Memory Management**: TodoWrite for task tracking, context compression
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
- No command line required

### Creator Workflow
Creators can develop Skills with **Claude Code** or **VS Code**, then package directly inside the WorkClaw app (no separate Studio client).

## Key Features

### Security & Privacy
- **Military-Grade Encryption**: AES-256-GCM with deterministic key derivation from username
- **Secure Workspace**: Configure trusted local folders for file operations
- **Permission Control**: Multi-layer validation for sensitive operations
- **No Cloud Dependency**: All processing happens locally

### Agent Capabilities
- **ReAct Loop Engine**: Advanced reasoning and action planning
- **Sub-Agent System**: Parallel task execution with isolated contexts
- **Context Compression**: Smart truncation to stay within token limits
- **Tool Registry**: Dynamic tool registration including MCP servers
- **Memory Persistence**: TodoWrite for task tracking across sessions

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

### Windows Auto Release (GitHub)

Tag-driven Windows release is enabled via GitHub Actions.

```bash
# 1) Keep versions aligned (apps/runtime/src-tauri/tauri.conf.json -> version)
# 2) Push a semantic version tag (triggers .github/workflows/release-windows.yml)
git tag v0.1.0
git push origin v0.1.0
```

Before publishing, CI validates that `tag(vX.Y.Z)` matches `tauri.conf.json` `version`.

### Installing a Skill

1. Open Runtime application
2. Click "Install Skill" or drag `.skillpack` file to window
3. Enter username (used for decryption key derivation)
4. Configure API keys if needed
5. Start chatting!

## Roadmap

### Milestone 1: Agent Runtime MVP ✨ (Current Focus)

**Core Agent Capabilities** (80% Complete):
- [x] ReAct loop executor with Tool trait abstraction
- [x] File operations: Read, Write, Glob, Grep, Edit
- [x] Bash/PowerShell execution with cross-platform support
- [x] Sub-Agent system (Task tool) for parallel task distribution
- [x] TodoWrite for task management and memory
- [x] Context compression (token budget management)
- [x] Web Search (DuckDuckGo)
- [x] WebFetch for URL content retrieval
- [x] AskUser for interactive user input
- [x] Tool output truncation (30k char limit)
- [x] Permission system (planned, multi-layer validation)
- [ ] Local secure workspace folder configuration
- [ ] MCP server dynamic registration UI (70% - backend done)

**Skill System**:
- [x] Skill YAML frontmatter parsing
- [x] .skillpack encryption/decryption (Rust)
- [x] Install, list, delete Skill commands
- [x] Dynamic Skill loading from `.claude/skills/` directory
- [x] Skill-based system prompt injection
- [ ] Hot reload during development

**Sidecar Integration**:
- [x] Node.js sidecar manager (lifecycle control)
- [x] Hono HTTP server (localhost:8765)
- [ ] Playwright browser automation (15+ tools)
- [x] MCP client integration (connect, list tools, invoke)
- [ ] Browser controller with normalized coordinates

**Multi-Model Support**:
- [x] Anthropic Messages API adapter (Claude models)
- [x] OpenAI-compatible adapter (GPT, MiniMax, DeepSeek, etc.)
- [x] Reasoning content filtering (DeepSeek, MiniMax)
- [x] Model configuration UI (API key, base URL, model name)
- [x] 9 provider presets (Claude, OpenAI, MiniMax, DeepSeek, Qwen, Moonshot, GLM, Yi, Custom)

**User Interface**:
- [x] Chat view with streaming messages
- [x] Markdown rendering with syntax highlighting
- [x] Tool call visualization cards
- [x] Sub-Agent nested display
- [x] Session history sidebar
- [x] Settings view (models, MCP servers)
- [x] AskUser interactive input cards
- [ ] File upload support
- [ ] Secure workspace configuration UI

### Milestone 2: Distribution & Updates 🚀

**Auto-Update**:
- [ ] Application auto-update mechanism (Tauri updater)
- [ ] Update server infrastructure
- [ ] Version check and notification
- [ ] Background download and install

**Skill Version Control**:
- [ ] Skill versioning system (semver)
- [ ] Upgrade/downgrade capabilities
- [ ] Dependency resolution
- [ ] Breaking change detection

**Packaging & Installers**:
- [ ] Windows: NSIS installer + code signing
- [ ] macOS: DMG + notarization
- [ ] Linux: AppImage + deb/rpm packages

**Distribution**:
- [ ] Official download server
- [ ] Mirror CDN setup
- [ ] Update channels (stable, beta, dev)

### Milestone 3: Ecosystem & Enterprise 🏢

**Creator Capabilities (Built into WorkClaw App)**:
- [ ] Monaco Editor integration
- [ ] Skill structure visual editor
- [ ] Embedded testing chat (Claude Code integration)
- [ ] One-click packaging UI
- [ ] Template library
- [ ] Publishing workflow

**Marketplace**:
- [ ] Web-based Skill marketplace
- [ ] Search and browse functionality
- [ ] User reviews and ratings
- [ ] Payment integration (Stripe/Alipay)
- [ ] Creator analytics dashboard

**Enterprise Features** (Inspired by enterprise agent architecture):
- [ ] User registration and authentication (JWT)
- [ ] Multi-tenant support (team workspaces)
- [ ] Unified model configuration management
- [ ] Usage quota and billing
- [ ] Admin dashboard with analytics
- [ ] SSO integration (LDAP, OAuth)
- [ ] Audit logging and compliance
- [ ] Private Skill repositories
- [ ] Role-based access control (RBAC)
- [ ] Resource usage monitoring

### Milestone 4: Agent Evolution & Ecosystem Integration 🧬

**EvoMap Integration** (Agent Self-Evolution):
- [ ] GEP (Genome Evolution Protocol) support
- [ ] Gene and Capsule data structures
- [ ] Six-step evolution cycle (Scan → Signal → Intent → Mutate → Validate → Solidify)
- [ ] A2A (Agent-to-Agent) protocol client
- [ ] Automatic capability inheritance from global gene pool
- [ ] Local evolution history and audit logs
- [ ] 70/30 resource allocation (repair vs exploration)

**OpenClaw Ecosystem Integration**:
- [ ] ClawHub Skill marketplace browser
- [ ] One-click Skill import from ClawHub
- [ ] Skill quality scoring and security scanning
- [ ] Community Skill discovery and installation

**Remote Access via IM** (Instant Messaging Integration):
- [ ] WeChat Work / DingTalk bot adapters
- [ ] Secure command relay with authentication
- [ ] Mobile-to-desktop Skill execution
- [ ] Task status notification and streaming results
- [ ] Multi-user permission isolation

## Why "WorkClaw"?

**Work**: Focuses on real task execution, delivery, and team collaboration  
**Claw**: Draws from the OpenClaw ecosystem and the "lobster crew" metaphor for controllable AI workers

Think of it as **"putting your AI employee team to work under your command."**

## Inspiration

Similar to how Cursor and Claude Code democratized AI-assisted coding, WorkClaw aims to democratize AI Skill distribution. Package your expertise once, distribute securely to thousands.

## Future Integration Roadmap

**Agent Evolution**:
- EvoMap's GEP (Genome Evolution Protocol) and A2A communication
- Agent capability inheritance and evolution mechanisms

**Ecosystem Integration**:
- ClawHub marketplace integration strategies
- Community Skill discovery and distribution

## ⚠️ Security Disclaimer

**IMPORTANT - READ BEFORE USE**

Desktop Agents have powerful capabilities including file system access and command execution. This creates inherent security risks:

- **Malicious Skills**: Third parties may distribute `.skillpack` files containing harmful code
- **System Access**: Installed Skills can read, modify, or delete files on your computer
- **Command Execution**: Skills can execute arbitrary shell commands with your user permissions
- **Data Exposure**: Skills may access sensitive data in your workspace folders

**By downloading, installing, or running this software, you acknowledge:**
1. You understand the security risks associated with desktop AI Agents
2. You will only install Skills from trusted sources
3. You will review and configure workspace permissions carefully
4. **The developers assume NO LIABILITY for any damages, data loss, or security breaches** resulting from the use of this software or any Skills installed through it

**If you do not agree to these terms, DO NOT download, install, or run this software.**

For security best practices, see [SECURITY.md](SECURITY.md).

## Advanced Technical Docs (Integrators & Maintainers)

The following docs are optional for most end users and mainly target integrators and maintainers:

- Feishu routing integration (CN): [docs/integrations/feishu-routing.md](docs/integrations/feishu-routing.md)
- Employee identity model (`employee_id`) (CN): [docs/architecture/employee-identity-model.md](docs/architecture/employee-identity-model.md)
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

## Acknowledgements

- Thanks to the [OpenClaw](https://github.com/openclaw/openclaw) open-source ecosystem for the foundational ideas and capabilities that WorkClaw builds on.

---

**Built with Tauri, React, and Rust** | Inspired by Claude Code, Gemini CLI, and the open-source Agent community
