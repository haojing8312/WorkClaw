# WorkClaw MVP 设计文档

**日期**：2026-02-19
**范围**：MVP — Studio（打包工具）+ Runtime（运行客户端）
**技术栈**：Tauri 2.0 + Rust + React + TypeScript

---

## 1. MVP 功能边界

### Studio（创作者端）
- 选择本地 Skill 目录（`.claude/skills/xxx/`）
- 文件树只读预览 + 自动读取 SKILL.md Front Matter 元数据
- 填写打包配置：Skill 名称、版本、作者、客户用户名
- 一键打包 → 生成 `.skillpack` 文件

**明确不在 MVP 范围**：Skill 编辑器、对话测试台、版本历史管理、批量导入

### Runtime（客户端）
- 安装 `.skillpack`（拖拽或文件选择）
- 输入用户名解密激活
- 已安装 Skill 列表（卡片/侧边栏）
- 对话界面（流式输出 + Markdown 渲染）
- 模型配置（API Key + Base URL 管理）

**明确不在 MVP 范围**：文件上传、导出对话、Skill 更新、本地模型

---

## 2. 项目结构

```
workclaw/
├── apps/
│   ├── studio/               # Studio 桌面应用（创作者）
│   │   ├── src/              # React + TypeScript 前端
│   │   └── src-tauri/        # Rust 后端
│   └── runtime/              # Runtime 桌面应用（客户）
│       ├── src/              # React + TypeScript 前端
│       └── src-tauri/        # Rust 后端
├── packages/
│   ├── ui/                   # 共享 shadcn/ui 组件
│   ├── skill-core/           # Skill 解析/验证（TypeScript）
│   └── skillpack-rs/         # 加密/解密/打包核心（Rust crate）
│       ├── src/
│       │   ├── pack.rs       # 打包逻辑
│       │   ├── unpack.rs     # 解包/安装逻辑
│       │   └── crypto.rs     # AES-256-GCM + 密钥派生
│       └── Cargo.toml
├── docs/
│   └── plans/
├── examples/                 # 示例 Skill 文件
├── package.json              # pnpm workspace 根
└── turbo.json                # Turborepo 配置
```

**关键原则**：`skillpack-rs` 是独立 Rust crate，被 Studio 和 Runtime 的 `src-tauri` 同时依赖，加密/解密逻辑只写一次。

---

## 3. 加密与打包方案

### 3.1 密钥派生

```
用户名（如 "alice"）
    ↓ PBKDF2-HMAC-SHA256
    ↓ salt = SHA256(skill_id + skill_name)   ← 固定 salt，确保幂等性
    ↓ iterations = 100_000
    → 32 字节 AES-256-GCM 密钥
```

**幂等性保证**：同一用户名 + 同一 Skill，永远派生相同密钥。创作者无需存储密钥。

### 3.2 .skillpack 文件结构

```
myskill.skillpack（zip 包）
├── manifest.json             # 明文，Runtime 读取元数据
├── icon.png                  # 明文，应用图标（可选）
└── encrypted/
    ├── SKILL.md.enc          # AES-256-GCM 加密
    ├── templates/
    │   └── *.md.enc
    └── examples/
        └── *.md.enc
```

### 3.3 manifest.json 结构

```json
{
  "id": "uuid-v4",
  "name": "合同审查助手",
  "description": "专业的合同风险识别和条款分析工具",
  "version": "1.0.0",
  "author": "张三",
  "recommended_model": "claude-3-5-sonnet-20241022",
  "tags": [],
  "created_at": "2026-02-19T00:00:00Z",
  "username_hint": "alice",
  "encrypted_verify": "<base64>"
}
```

**`encrypted_verify`**：用派生密钥加密固定字符串 `"SKILLMINT_OK"`，Runtime 解密此字段验证用户名是否正确，给出友好报错而非乱码。

### 3.4 crypto.rs 接口设计

```rust
pub fn derive_key(username: &str, skill_id: &str, skill_name: &str) -> [u8; 32];
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>>;
pub fn decrypt(ciphertext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>>;
pub fn make_verify_token(key: &[u8; 32]) -> Result<String>;    // base64
pub fn check_verify_token(token: &str, key: &[u8; 32]) -> bool;
```

---

## 4. Studio 设计

### 4.1 用户流程

```
1. 选择 Skill 目录
   └── 文件选择器 → 读取目录结构 → 验证 SKILL.md 存在

2. 预览与填写元数据
   ├── 左侧：文件树（只读）
   └── 右侧表单：名称 / 版本 / 作者 / 客户用户名

3. 一键打包
   └── skillpack-rs::pack() → 保存对话框 → 生成 .skillpack
```

### 4.2 Tauri Commands（Studio）

```rust
#[tauri::command]
async fn select_skill_dir() -> Result<SkillDirInfo>;
// 返回：{ files: Vec<String>, front_matter: FrontMatter }

#[tauri::command]
async fn pack_skill(config: PackConfig) -> Result<()>;
// PackConfig: { dir_path, name, version, author, username, output_path }
```

### 4.3 界面布局

```
┌─────────────────────────────────────────────────────┐
│  WorkClaw Studio                          [− □ ×]   │
├─────────────────────────────────────────────────────┤
│  [选择 Skill 目录]  已选择：/skills/contract-review  │
│                                                     │
│  ┌──────────────────┐  ┌───────────────────────┐    │
│  │ 文件树（只读）     │  │ 打包配置               │    │
│  │                  │  │                       │    │
│  │ 📄 SKILL.md      │  │ Skill 名称             │    │
│  │ 📁 templates/    │  │ [合同审查助手        ]  │    │
│  │   📄 outline.md  │  │                       │    │
│  │ 📁 examples/     │  │ 版本号                 │    │
│  │   📄 sample.md   │  │ [1.0.0              ]  │    │
│  │                  │  │                       │    │
│  │                  │  │ 作者                  │    │
│  │                  │  │ [张三               ]  │    │
│  │                  │  │                       │    │
│  │                  │  │ 客户用户名（解密密钥）  │    │
│  │                  │  │ [alice              ]  │    │
│  │                  │  │                       │    │
│  │                  │  │ ℹ️ 客户需输入此用户名   │    │
│  │                  │  │    才能解锁 Skill       │    │
│  └──────────────────┘  │                       │    │
│                         │  [  一键打包  ]        │    │
│                         └───────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

---

## 5. Runtime 设计

### 5.1 用户流程

```
1. 安装 Skill
   └── 拖拽 .skillpack / 点击"安装" → 输入用户名 → 验证 → 写入 SQLite

2. 使用 Skill
   └── 侧边栏选择 Skill → 对话界面 → 流式输出

3. 模型配置
   └── 设置 → 添加模型 → API Key + Base URL → 测试连接
```

### 5.2 本地数据库（SQLite）

```sql
CREATE TABLE installed_skills (
    id TEXT PRIMARY KEY,
    manifest TEXT NOT NULL,       -- JSON
    installed_at DATETIME,
    last_used_at DATETIME,
    skill_enc BLOB NOT NULL,      -- 加密内容（整个 encrypted/ 目录打包）
    username TEXT NOT NULL        -- 存储用户名，运行时派生密钥
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    skill_id TEXT NOT NULL,
    title TEXT,
    created_at DATETIME,
    model_id TEXT NOT NULL
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,           -- 'user' | 'assistant'
    content TEXT NOT NULL,
    created_at DATETIME
);

CREATE TABLE model_configs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    api_format TEXT NOT NULL,     -- 'anthropic' | 'openai'
    base_url TEXT NOT NULL,
    model_name TEXT NOT NULL,
    is_default BOOLEAN DEFAULT FALSE
);
```

**API Key** 存储在系统 Keychain（keyring），不入库。

### 5.3 Tauri Commands（Runtime）

```rust
#[tauri::command]
async fn install_skill(pack_path: String, username: String) -> Result<SkillManifest>;

#[tauri::command]
async fn list_skills() -> Result<Vec<SkillManifest>>;

#[tauri::command]
async fn delete_skill(skill_id: String) -> Result<()>;

#[tauri::command]
async fn send_message(skill_id: String, session_id: String, message: String, model_id: String) -> Result<()>;
// 流式输出通过 Tauri Event emit 到前端

#[tauri::command]
async fn save_model_config(config: ModelConfig) -> Result<()>;

#[tauri::command]
async fn test_connection(config: ModelConfig) -> Result<bool>;
```

### 5.4 模型适配层

```rust
trait ModelAdapter: Send + Sync {
    async fn chat_stream(
        &self,
        messages: Vec<Message>,
        system_prompt: &str,
        options: ChatOptions,
        on_token: impl Fn(String),
    ) -> Result<()>;

    async fn test_connection(&self) -> Result<bool>;
}

struct AnthropicAdapter { api_key: String }
struct OpenAICompatAdapter { base_url: String, api_key: String, model: String }
```

### 5.5 界面布局

```
┌──────────────┬──────────────────────────────────────┐
│ 已安装 Skill  │ 合同审查助手          [新建会话]       │
│              ├──────────────────────────────────────┤
│ 🤖 合同审查 ● │                                      │
│ 🤖 营销文案   │  [AI] 你好！请粘贴需要审查的合同内容。  │
│              │                                      │
│              │  [我] 以下是合同第三条...              │
│              │                                      │
│              │  [AI] 分析结果：▌（流式输出中）        │
│              │                                      │
│              ├──────────────────────────────────────┤
│ [+ 安装]     │ [输入消息...              ] [发送]    │
│ [⚙ 设置]    │                                      │
└──────────────┴──────────────────────────────────────┘
```

---

## 6. 共享 UI 组件（packages/ui）

两个应用共享的组件：
- `ChatMessage` — 消息气泡，Markdown 渲染
- `ModelSelector` — 模型下拉选择
- `FileTree` — 文件树展示（只读）
- `InstallDialog` — 安装 + 用户名输入弹窗

---

## 7. 技术依赖清单

### Rust 依赖
| crate | 用途 |
|-------|------|
| `aes-gcm` | AES-256-GCM 加密 |
| `pbkdf2` + `hmac` + `sha2` | 密钥派生 |
| `zip` | .skillpack 打包/解包 |
| `uuid` | Skill ID 生成 |
| `sqlx` | SQLite 操作 |
| `reqwest` | HTTP 请求（模型 API）|
| `keyring` | 系统 Keychain 存储 |
| `tauri` | 桌面应用框架 |
| `serde` / `serde_json` | 序列化 |

### 前端依赖
| 包 | 用途 |
|----|------|
| `react` + `typescript` | UI 框架 |
| `tailwindcss` | 样式 |
| `shadcn/ui` | 组件库 |
| `@tauri-apps/api` | Tauri IPC |
| `react-markdown` | Markdown 渲染 |
| `react-syntax-highlighter` | 代码高亮 |

---

## 8. 非功能性目标（MVP）

| 指标 | 目标 |
|------|------|
| 打包速度 | < 3 秒（单个 Skill） |
| 安装速度 | < 1 秒 |
| 首条消息流式开始 | < 1 秒 |
| Runtime 安装包 | < 30MB |
| Studio 安装包 | < 50MB |

---

*设计文档版本：v1.0 | 2026-02-19*
