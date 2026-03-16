# 浏览器自动化集成（本地实现）

本文档说明 WorkClaw 当前版本的浏览器自动化能力（纯本地 sidecar + Playwright），以及面向 OpenClaw browser skill 的 P0 兼容层。

当前实现不依赖任何外部 OpenClaw 服务，也不需要 `openclaw browser start` 一类额外进程。

## 能力范围

- `browser_launch` 本地启动浏览器
- `browser_snapshot` 生成页面快照与 `ref -> selector` 映射
- `browser_act` 基于 `ref` 或 `selector` 执行动作
- 兼容原有 `browser_*` 工具（navigate/click/type/scroll/...）
- 新增统一 `browser` 兼容工具，面向 OpenClaw 风格 skill

## OpenClaw Browser Compat（P0）

P0 兼容层的目标是跑通 `xiaohongshu-ops-skill` 一类 browser-heavy skill 的“半程发布链路”，不是完整复刻 OpenClaw runtime。

### 统一工具名

运行时会在会话级动态注册以下兼容工具名：

- `browser`
- `read`
- `find`
- `ls`
- `exec`

这些别名只在聊天运行时暴露，不会污染 `ToolRegistry::with_standard_tools()` 的静态标准工具集合。

### browser(action=...) 支持范围

P0 已支持：

- `start`
- `status`
- `profiles`
- `tabs`
- `open`
- `focus`
- `snapshot`
- `act`
- `upload`

### profile 约束

P0 仅支持：

- `profile="openclaw"`

不支持：

- `profile="chrome"`
- OpenClaw relay / node / sandbox 模式

### 持久 profile 与 targetId

- `openclaw` profile 使用持久化 Playwright context，登录态会保存在 WorkClaw 自己的目录下
- compat 层维护真实 `targetId -> page` 映射
- `snapshot` / `act` / `upload` 都可按 `targetId` 作用于指定 tab

### 上传兼容

- `browser(action="upload")` 支持普通本地文件路径
- 也兼容 `/tmp/openclaw/uploads/...` 这种 OpenClaw 风格路径
- WorkClaw 会把上传文件映射/暂存到自有 staging 目录后再交给 Playwright

### 诊断日志

compat 层会在 sidecar 日志中输出：

- `action`
- `profile`
- `targetId`

便于排查 OpenClaw 风格 skill 的运行问题。

浏览器自动化与 IM 连接器解耦：

- 浏览器工具仍由 `apps/runtime/sidecar/src/browser.ts` 提供
- IM 多渠道扩展走 `apps/runtime/sidecar/src/adapters/`
- 两者共享 sidecar 进程，但没有运行时耦合

## 启动参数

调用 `browser_launch` 时可传入：

```json
{
  "headless": false,
  "viewport": { "width": 1280, "height": 720 }
}
```

## 快照与动作

### browser_snapshot

示例：

```json
{
  "format": "ai",
  "interactive": true,
  "limit": 200
}
```

返回结果包含：

- `refs`: 如 `{ "e1": "#submit", "e2": "body > form:nth-of-type(1) > input:nth-of-type(1)" }`
- `snapshot`: 文本快照，如 `[e1] <button> "提交"`

### browser_act

示例（按 ref 点击）：

```json
{
  "kind": "click",
  "ref": "e1"
}
```

示例（输入并回车提交）：

```json
{
  "kind": "type",
  "ref": "e2",
  "text": "user@example.com",
  "submit": true
}
```

支持动作：

- `click`
- `type`
- `press`
- `hover`
- `drag`
- `select`
- `fill`
- `resize`
- `wait`
- `evaluate`
- `close`

## 状态检查

`browser_get_state` 返回示例：

```json
{
  "running": true,
  "url": "https://example.com/",
  "title": "Example Domain",
  "backend": "playwright",
  "snapshotRefs": 12
}
```

## 一键测试命令

在仓库根目录执行：

- `pnpm test:sidecar`
- `pnpm test:browser-automation`

## 安全门禁（PermissionMode）

在 `Default` 与 `AcceptEdits` 模式下，以下浏览器工具会触发用户确认：

- `browser_act`
- `browser_click`
- `browser_type`
- `browser_press_key`
- `browser_evaluate`
- `browser_launch`
- `browser_navigate`

`Unrestricted` 模式下不触发确认。
