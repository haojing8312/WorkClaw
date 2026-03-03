# Agent 工具补全设计方案

**日期**: 2026-02-24
**状态**: 已批准
**目标**: 将 WorkClaw Agent 从 8 个注册工具扩展到 37 个，覆盖文件操作、Shell 进程管理、浏览器自动化、系统工具等完整桌面 Agent 能力。

## 背景

对比 MiniMax Agent（60+ IPC 通道）后发现 WorkClaw 在以下方面存在显著差距：

- 文件操作缺少 `listDir`, `delete`, `move`, `copy`, `stat`
- Shell 无后台进程管理能力
- 浏览器自动化工具为零（sidecar 基础设施已有但未暴露）
- 5 个已实现的工具未注册到 Agent
- 缺少系统级工具

## 分层实现策略

采用 5 层渐进式实现，每层独立可测试。

### L1: 注册已有工具

将 5 个已编写但未注册的工具加入默认工具集：

| 工具 | 文件 | 构造依赖 |
|------|------|---------|
| `memory` | `memory_tool.rs` | `memory_dir: PathBuf` |
| `ask_user` | `ask_user.rs` | `AskUserResponder` |
| `web_search` | `web_search.rs` | 无 |
| `task` | `task_tool.rs` | 无 |
| `compact` | `compact_tool.rs` | 无 |

> `with_file_tools()` 方法需重命名为 `with_standard_tools()` 并调整签名以接收依赖参数。

### L2: 文件扩展工具（5 个新工具）

| 工具名 | 文件 | 功能 |
|--------|------|------|
| `list_dir` | `list_dir.rs` | 列出目录内容，返回文件名/类型/大小 |
| `file_stat` | `file_stat.rs` | 获取文件元信息（大小、修改时间、权限） |
| `file_delete` | `file_delete.rs` | 删除文件或空目录 |
| `file_move` | `file_move.rs` | 移动/重命名文件或目录 |
| `file_copy` | `file_copy.rs` | 复制文件或目录（递归） |

所有工具复用 `ctx.check_path()` 进行路径安全检查。`file_delete` 仅支持删除文件和**空目录**，防止误删。

### L3: Shell 进程管理

#### 扩展 `bash` 工具

新增 `background` 参数：

```json
{
    "command": "npm run build",
    "timeout_ms": 120000,
    "background": true
}
```

后台模式返回 `process_id`，不等待命令完成。

#### 新增进程管理工具

| 工具名 | 功能 |
|--------|------|
| `bash_output` | 获取后台进程输出，支持 `block: true` 等待完成 |
| `bash_kill` | 终止后台进程 |

#### ProcessManager 架构

```
ProcessManager (Arc<Mutex<HashMap<String, BackgroundProcess>>>)
├── spawn(command, work_dir) → process_id
├── get_output(process_id, block) → stdout/stderr/status
├── kill(process_id) → ok/error
└── cleanup() → 移除已完成的旧进程（保留最近 30 个）
```

- 全局单例，通过 `lazy_static!` 实现
- 后台进程输出用独立线程持续读取到内存缓冲区（最大 5000 行）
- `BackgroundProcess` 结构：`pid`, `child`, `stdout_buffer`, `stderr_buffer`, `status`, `started_at`

### L4: Sidecar 浏览器自动化（15 个工具）

#### 架构

```
Rust Agent ──HTTP──▶ Sidecar (localhost:8765)
                        └── /api/browser/* (15 个端点)
```

#### 浏览器工具清单

| # | 端点 | Agent 工具名 | 参数 | 功能 |
|---|------|-------------|------|------|
| 1 | `/api/browser/launch` | `browser_launch` | `headless?`, `viewport?` | 启动浏览器实例 |
| 2 | `/api/browser/navigate` | `browser_navigate` | `url` | 导航到 URL（30s 超时） |
| 3 | `/api/browser/click` | `browser_click` | `selector?`, `x?`, `y?` | 点击（CSS 选择器或坐标） |
| 4 | `/api/browser/type` | `browser_type` | `selector`, `text`, `delay?` | 输入文本（支持逐字符延迟） |
| 5 | `/api/browser/scroll` | `browser_scroll` | `direction`, `amount?` | 滚动（up/down/to_top/to_bottom） |
| 6 | `/api/browser/hover` | `browser_hover` | `selector?`, `x?`, `y?` | 悬停元素 |
| 7 | `/api/browser/press_key` | `browser_press_key` | `key`, `modifiers?` | 键盘按键（支持组合键） |
| 8 | `/api/browser/screenshot` | `browser_screenshot` | `selector?`, `full_page?` | 截图（返回 base64 或文件路径） |
| 9 | `/api/browser/get_dom` | `browser_get_dom` | `selector?`, `max_depth?` | 提取简化 DOM 结构 |
| 10 | `/api/browser/evaluate` | `browser_evaluate` | `script` | 在页面上下文执行 JS |
| 11 | `/api/browser/wait_for` | `browser_wait_for` | `selector?`, `condition?`, `timeout?` | 等待选择器出现或 JS 条件满足 |
| 12 | `/api/browser/go_back` | `browser_go_back` | 无 | 浏览器后退 |
| 13 | `/api/browser/go_forward` | `browser_go_forward` | 无 | 浏览器前进 |
| 14 | `/api/browser/reload` | `browser_reload` | 无 | 刷新页面 |
| 15 | `/api/browser/get_state` | `browser_get_state` | 无 | 获取当前 URL/标题/加载状态/历史 |

#### Stealth 反检测集成

在 Sidecar 中使用 `playwright-extra` + `puppeteer-extra-plugin-stealth`：

```typescript
import { chromium } from "playwright-extra";
import stealth from "puppeteer-extra-plugin-stealth";

chromium.use(stealth());
const browser = await chromium.launch({ headless: true });
```

覆盖：navigator.webdriver 隐藏、WebGL/Canvas 指纹混淆、WebRTC 防泄漏、Permissions API mock 等。

#### Rust 侧动态注册

浏览器工具通过 `register_browser_tools()` 函数批量注册：

```rust
pub fn register_browser_tools(registry: &ToolRegistry, sidecar_url: &str) {
    let tools = vec![
        ("browser_navigate", "导航到指定 URL", "/api/browser/navigate", schema_navigate()),
        // ... 其余 14 个工具
    ];
    for (name, desc, endpoint, schema) in tools {
        registry.register(Arc::new(SidecarBridgeTool::new(
            sidecar_url.into(), endpoint.into(), name.into(), desc.into(), schema,
        )));
    }
}
```

### L5: 系统工具（2 个）

| 工具名 | 文件 | 功能 | 实现方式 |
|--------|------|------|---------|
| `screenshot` | `screenshot.rs` | 全屏截图 | 系统命令（Windows: `snippingtool`, macOS: `screencapture`, Linux: `gnome-screenshot`） |
| `open_in_folder` | `open_in_folder.rs` | 在文件管理器中显示文件 | `opener::reveal` 或 `open::that` |

## 注册表重构

```rust
impl ToolRegistry {
    /// 创建包含所有标准工具的注册表
    pub fn with_standard_tools() -> Self {
        // L2 文件工具 + L1 基础工具
    }

    /// 注册高级工具（需要外部依赖）
    pub fn register_advanced_tools(&self, memory_dir: PathBuf, responder: AskUserResponder) {
        // memory, ask_user, web_search, task, compact
    }

    /// 注册浏览器工具（需要 sidecar 运行）
    pub fn register_browser_tools(&self, sidecar_url: &str) {
        // 15 个浏览器工具
    }

    /// 注册系统工具
    pub fn register_system_tools(&self) {
        // screenshot, open_in_folder
    }
}
```

## 最终工具清单（37 个）

| 类别 | 数量 | 工具列表 |
|------|------|---------|
| 文件操作 | 10 | read_file, write_file, edit, glob, grep, list_dir, file_stat, file_delete, file_move, file_copy |
| Shell | 3 | bash（含后台模式）, bash_output, bash_kill |
| 信息获取 | 2 | web_fetch, web_search |
| 任务管理 | 2 | todo_write, task |
| 用户交互 | 1 | ask_user |
| 知识管理 | 1 | memory |
| 上下文管理 | 1 | compact |
| 浏览器自动化 | 15 | browser_launch, browser_navigate, browser_click, browser_type, browser_scroll, browser_hover, browser_press_key, browser_screenshot, browser_get_dom, browser_evaluate, browser_wait_for, browser_go_back, browser_go_forward, browser_reload, browser_get_state |
| 系统 | 2 | screenshot, open_in_folder |

## 测试策略

- L1: 验证工具注册后 `get_tool_definitions()` 返回完整列表
- L2: 每个文件工具有独立的单元测试（临时目录）
- L3: ProcessManager 测试（spawn + output + kill），后台进程超时清理
- L4: Sidecar HTTP 端点集成测试（需要 sidecar 运行），Mock 测试（不需要 sidecar）
- L5: 系统工具基本功能测试

## 不在范围内

- 文件选择对话框（selectDirectory/selectFile）— Agent 通常知道路径
- Tab 管理（create/close/activate）— 单 tab 足够
- 应用更新/部署工具 — 独立功能
- 日志上传工具 — 运维功能
