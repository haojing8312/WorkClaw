# 飞书浏览器配置向导

本文档说明 WorkClaw 正在接入中的 `Chrome 扩展 + 本地桥接 + 默认浏览器` 飞书配置向导能力。

目标不是在桌面端内嵌飞书后台，而是在用户自己的 Chrome 中完成：

- 打开飞书开放平台
- 检测是否已登录
- 识别企业自建应用配置步骤
- 读取 `App ID / App Secret`
- 回传到本地 WorkClaw

## 当前状态

截至 `2026-03-12`，本仓库已完成以下基础能力：

- 浏览器桥共享协议与 Feishu setup 状态模型
- Tauri 侧 Feishu setup session store
- Chrome 扩展骨架与页面 detector
- 凭证页 `App ID / App Secret` 提取函数
- `workclaw_session_id` URL 透传与默认浏览器打开
- content script 自动读取凭据并发送到扩展 runtime
- background script 自动接收凭据并优先走 `connectNative`，失败回退到本地 HTTP bridge
- native-host framing 与本地 HTTP bridge client
- sidecar 已提供 `/api/browser-bridge/native-message` 真实接收路由
- 桌面端会启动本地 Rust callback server，并通过环境变量把 callback URL 注入 sidecar
- 设置页中的“浏览器桥接安装”与“飞书浏览器配置向导”入口
- 设置页可读取浏览器桥接安装状态，并在“等待启用”阶段自动轮询
- 扩展启用后会发送最小 `bridge.hello` 握手，用于把安装状态切换为 `connected`
- Windows 下 Chrome native host manifest 生成脚本

尚未完全接通的部分：

- 飞书后台真实页面的稳定 selector/step detector
- Windows 安装命令的真实文件写入与扩展目录准备仍是 stub
- native host 可执行入口与安装后的真实浏览器端到端联调

## 目录结构

- 扩展侧：
  - `apps/runtime/src/browser-bridge/chrome-extension/`
- 本地桥接：
  - `apps/runtime/src/browser-bridge/native-host/`
- Tauri setup 状态：
  - `apps/runtime/src-tauri/src/commands/feishu_browser_setup.rs`
- 安装脚本：
  - `scripts/install-chrome-native-host.mjs`

## 一键安装链路（当前设计）

产品入口位于现有 `设置` 页签，分成两段：

1. `安装浏览器桥接`
2. `启动飞书浏览器配置`

第一段负责自动准备本地桥接环境，并把用户带到 Chrome 的最后一步启用；第二段负责真正进入飞书后台配置流程。

### 1. 安装 Chrome 扩展

当前仓库中已有 Manifest V3 骨架：

- `apps/runtime/src/browser-bridge/chrome-extension/manifest.json`

后续需要把这套代码打包为实际扩展产物，再在 Chrome 中加载。

普通用户版的一键安装目标是：

- 自动安装 native host
- 自动准备 WorkClaw 扩展目录
- 自动打开 Chrome 扩展页
- 在桌面端持续显示 `未安装 -> 等待启用 -> 已连接`

第一版仍然保留最后一步人工确认：

- 用户需要在 Chrome 中开启开发者模式
- 用户需要手动点击“加载已解压的扩展程序”
- 用户需要选择 WorkClaw 已准备好的扩展目录

### 2. 安装 Native Messaging Host Manifest

当前提供的脚本：

```bash
node scripts/install-chrome-native-host.mjs "<Chrome User Data Dir>" "<Command Or Launcher Path>" "<Extension Origin>" [nodePath scriptPath baseUrl]
```

当前仓库还提供了 native host 可执行脚本与 Windows launcher 模板：

- native host 脚本：
  - `scripts/workclaw-chrome-native-host.mjs`
- launcher 模板函数：
  - `buildWindowsNativeHostLauncher(...)` in `scripts/install-chrome-native-host.mjs`

推荐在 Windows 上直接让安装脚本生成 `.cmd` launcher，再把 manifest 的 `path` 指向这个 launcher。

示例：

```bash
node scripts/install-chrome-native-host.mjs ^
  "C:\Users\<用户名>\AppData\Local\Google\Chrome\User Data" ^
  "C:\WorkClaw\native-host.cmd" ^
  "chrome-extension://abcdefghijklmnop/" ^
  "C:\Program Files\nodejs\node.exe" ^
  "E:\code\yzpd\workclaw\scripts\workclaw-chrome-native-host.mjs" ^
  "http://127.0.0.1:4312"
```

传入 `nodePath + scriptPath` 时，脚本会同时：

- 写出 `C:\WorkClaw\native-host.cmd`
- 写出 `<Chrome User Data Dir>\NativeMessagingHosts\dev.workclaw.runtime.json`

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

1. 用户在 WorkClaw 设置页点击“安装浏览器桥接”
2. WorkClaw 安装 native host，并准备扩展目录
3. WorkClaw 打开 Chrome 扩展页和扩展目录
4. 用户在 Chrome 中开启开发者模式并加载 WorkClaw 扩展
5. 扩展 background 启动后发送 `bridge.hello`
6. sidecar / callback server 标记本地浏览器桥接为 `connected`
7. 设置页显示“浏览器桥接已启用，可以开始飞书配置”
8. 用户点击“启动飞书浏览器配置”
9. WorkClaw 创建本地 setup session
10. 用户默认 Chrome 打开 `https://open.feishu.cn/`
   - 实际 URL 现会带上 `?workclaw_session_id=<session_id>`
11. 扩展识别当前页面：
   - 未登录：提示用户先登录
   - 凭证页：读取 `App ID / App Secret`
12. content script 通过扩展 runtime 把凭据发送给 background
13. background 优先通过 `chrome.runtime.connectNative` 把凭据发送到本地；若不可用，则回退到 sidecar 的 `/api/browser-bridge/native-message`
14. sidecar 在配置了 `WORKCLAW_BROWSER_BRIDGE_CALLBACK_URL` 时，会把 envelope 转发到桌面端 callback server
15. 桌面端 callback server 调用 `FeishuBrowserSetupStore::report_credentials_and_bind(...)`
16. WorkClaw 写入现有飞书设置键：
   - `feishu_app_id`
   - `feishu_app_secret`
17. UI 状态推进到 `ENABLE_LONG_CONNECTION`

## 当前限制

- 凭证提取当前同时支持测试用 `data-field` 与简单的标签/相邻文本模式，但还没覆盖真实飞书后台全部 DOM 变体
- native host transport 已有 runner 脚本、launcher 安装能力、sidecar 接收端和 Rust callback server，但尚未做真实 Chrome 扩展安装后的端到端联调
- 当前 `install_browser_bridge` 仍是命令面 stub，Windows 真正写 manifest / launcher / 扩展目录准备将在后续任务补齐
- 浏览器桥接“已连接”基于最近一次 hello 心跳判断，不是完整的健康探针
- Windows 环境下 Rust 验证仍建议使用独立 `CARGO_HOME`，以避开系统 Cargo cache 锁争用

## 安全边界

- 目标域名限制为：`https://open.feishu.cn/*`
- 敏感凭据只允许发送到本地 WorkClaw
- 扩展不应持久化 `App Secret`
- 本地日志需要脱敏

## 当前验证命令

前端测试：

```bash
CI=1 pnpm --dir apps/runtime test src/browser-bridge/shared/__tests__/protocol.test.ts src/browser-bridge/shared/__tests__/feishu-setup.test.ts src/browser-bridge/native-host/__tests__/native-host.test.ts src/browser-bridge/chrome-extension/__tests__/feishu-detector.test.ts src/browser-bridge/chrome-extension/__tests__/content.test.ts src/browser-bridge/chrome-extension/__tests__/background.test.ts src/components/employees/__tests__/BrowserBridgeInstallCard.test.tsx src/components/employees/__tests__/FeishuBrowserSetupView.test.tsx src/components/employees/__tests__/EmployeeHubView.browser-setup.test.tsx
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

已知本机环境说明：

- 这台 Windows 开发机上，Vitest 需要显式设置 `CI=1` 才会稳定退出
- `apps/runtime/sidecar/test/browser-bridge-endpoints.test.ts` 当前可能受到本机 `tsx + playwright-core` 环境异常影响，需和代码断言失败区分开

## 后续优先事项

1. 把扩展的页面 detector 从测试选择器推进到真实飞书后台 DOM
2. 接入真实的 `chrome.runtime.connectNative`
3. 解决 `lib.rs` 的命令注册编译问题
4. 增加真实浏览器安装与加载流程说明
