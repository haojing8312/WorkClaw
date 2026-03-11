# 飞书浏览器配置向导

本文档说明 WorkClaw 正在接入中的 `Chrome 扩展 + 本地桥接 + 默认浏览器` 飞书配置向导能力。

目标不是在桌面端内嵌飞书后台，而是在用户自己的 Chrome 中完成：

- 打开飞书开放平台
- 检测是否已登录
- 识别企业自建应用配置步骤
- 读取 `App ID / App Secret`
- 回传到本地 WorkClaw

## 当前状态

截至 `2026-03-11`，本仓库已完成以下基础能力：

- 浏览器桥共享协议与 Feishu setup 状态模型
- Tauri 侧 Feishu setup session store
- Chrome 扩展骨架与页面 detector
- 凭证页 `App ID / App Secret` 提取函数
- `workclaw_session_id` URL 透传与默认浏览器打开
- content script 自动读取凭据并发送到扩展 runtime
- background script 自动接收凭据并优先走 `connectNative`，失败回退到本地 HTTP bridge
- native-host framing 与本地 HTTP bridge client
- 员工中心设置页中的“飞书浏览器配置向导”入口
- Windows 下 Chrome native host manifest 生成脚本

尚未完全接通的部分：

- 飞书后台真实页面的稳定 selector/step detector
- native host 可执行入口与安装后的端到端联调
- background 收到凭据后自动调用本地 Tauri 命令完成 session 推进

## 目录结构

- 扩展侧：
  - `apps/runtime/src/browser-bridge/chrome-extension/`
- 本地桥接：
  - `apps/runtime/src/browser-bridge/native-host/`
- Tauri setup 状态：
  - `apps/runtime/src-tauri/src/commands/feishu_browser_setup.rs`
- 安装脚本：
  - `scripts/install-chrome-native-host.mjs`

## 安装链路（当前设计）

### 1. 安装 Chrome 扩展

当前仓库中已有 Manifest V3 骨架：

- `apps/runtime/src/browser-bridge/chrome-extension/manifest.json`

后续需要把这套代码打包为实际扩展产物，再在 Chrome 中加载。

### 2. 安装 Native Messaging Host Manifest

当前提供的脚本：

```bash
node scripts/install-chrome-native-host.mjs "<Chrome User Data Dir>" "<Native Host Command>" "<Extension Origin>"
```

当前仓库还提供了 native host 可执行脚本与 Windows launcher 模板：

- native host 脚本：
  - `scripts/workclaw-chrome-native-host.mjs`
- launcher 模板函数：
  - `buildWindowsNativeHostLauncher(...)` in `scripts/install-chrome-native-host.mjs`

推荐在 Windows 上用一个 `.cmd` launcher 包住 Node 脚本，再把 manifest 的 `path` 指向这个 launcher。

示例：

```bash
node scripts/install-chrome-native-host.mjs ^
  "C:\Users\<用户名>\AppData\Local\Google\Chrome\User Data" ^
  "C:\WorkClaw\native-host.cmd" ^
  "chrome-extension://abcdefghijklmnop/"
```

其中 `C:\WorkClaw\native-host.cmd` 的内容建议类似：

```bat
@echo off
set "WORKCLAW_BROWSER_BRIDGE_BASE_URL=http://127.0.0.1:4312"
"C:\Program Files\nodejs\node.exe" "E:\code\yzpd\workclaw\scripts\workclaw-chrome-native-host.mjs"
```

脚本会生成：

```text
<Chrome User Data Dir>\NativeMessagingHosts\dev.workclaw.runtime.json
```

manifest 内容包含：

- `name = dev.workclaw.runtime`
- `type = stdio`
- `path = <Native Host Command>`
- `allowed_origins = [<Extension Origin>]`

## 预期用户路径

1. 用户在 WorkClaw 员工中心设置页点击“启动飞书浏览器配置”
2. WorkClaw 创建本地 setup session
3. 用户默认 Chrome 打开 `https://open.feishu.cn/`
   - 实际 URL 现会带上 `?workclaw_session_id=<session_id>`
4. 扩展识别当前页面：
   - 未登录：提示用户先登录
   - 凭证页：读取 `App ID / App Secret`
5. content script 通过扩展 runtime 把凭据发送给 background
6. background 优先通过 `chrome.runtime.connectNative` 把凭据发送到本地；若不可用，则回退到本地 HTTP bridge
7. WorkClaw 写入现有飞书设置键：
   - `feishu_app_id`
   - `feishu_app_secret`
8. UI 状态推进到 `ENABLE_LONG_CONNECTION`

## 当前限制

- 凭证提取当前同时支持测试用 `data-field` 与简单的标签/相邻文本模式，但还没覆盖真实飞书后台全部 DOM 变体
- native host transport 已有 helper 和 listener，但尚未与安装后的本地 host 进程做端到端联调
- 扩展 background 当前把凭据发给本地 bridge 后，还没有自动轮询或订阅 Tauri session 状态变化
- Windows 环境下 Rust 验证仍建议使用独立 `CARGO_HOME`，以避开系统 Cargo cache 锁争用

## 安全边界

- 目标域名限制为：`https://open.feishu.cn/*`
- 敏感凭据只允许发送到本地 WorkClaw
- 扩展不应持久化 `App Secret`
- 本地日志需要脱敏

## 当前验证命令

前端测试：

```bash
pnpm --dir apps/runtime test src/browser-bridge/shared/__tests__/protocol.test.ts src/browser-bridge/shared/__tests__/feishu-setup.test.ts src/browser-bridge/native-host/__tests__/native-host.test.ts src/browser-bridge/chrome-extension/__tests__/feishu-detector.test.ts src/browser-bridge/chrome-extension/__tests__/content.test.ts src/browser-bridge/chrome-extension/__tests__/background.test.ts src/components/employees/__tests__/FeishuBrowserSetupView.test.tsx src/components/employees/__tests__/EmployeeHubView.browser-setup.test.tsx
```

脚本测试：

```bash
node --test scripts/install-chrome-native-host.test.mjs
```

Rust 测试：

```bash
cd apps/runtime/src-tauri
cargo test --test test_feishu_browser_setup --test test_feishu_browser_setup_binding -- --nocapture
```

## 后续优先事项

1. 把扩展的页面 detector 从测试选择器推进到真实飞书后台 DOM
2. 接入真实的 `chrome.runtime.connectNative`
3. 解决 `lib.rs` 的命令注册编译问题
4. 增加真实浏览器安装与加载流程说明
