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
- native-host framing 与本地 HTTP bridge client
- 员工中心设置页中的“飞书浏览器配置向导”入口
- Windows 下 Chrome native host manifest 生成脚本

尚未完全接通的部分：

- Tauri `invoke_handler` 中正式注册 `start_feishu_browser_setup` 等命令
- 真正的 Chrome `connectNative` 通道
- 飞书后台真实页面的稳定 selector/step detector

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

示例：

```bash
node scripts/install-chrome-native-host.mjs ^
  "C:\Users\<用户名>\AppData\Local\Google\Chrome\User Data" ^
  "C:\WorkClaw\native-host.cmd" ^
  "chrome-extension://abcdefghijklmnop/"
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
4. 扩展识别当前页面：
   - 未登录：提示用户先登录
   - 凭证页：读取 `App ID / App Secret`
5. 扩展把凭据发送到本地 bridge
6. WorkClaw 写入现有飞书设置键：
   - `feishu_app_id`
   - `feishu_app_secret`
7. UI 状态推进到 `ENABLE_LONG_CONNECTION`

## 当前限制

- 目前凭证提取使用的是测试用 `data-field="app-id"` / `data-field="app-secret"` 选择器
- 尚未接入真实飞书后台 DOM
- 当前桥接仍通过本地 HTTP client 抽象模拟，尚未切到 Chrome 原生 `connectNative`
- `start_feishu_browser_setup` 等命令尚未正式挂入 `lib.rs` 的 `invoke_handler`
- 在当前 Windows 环境下，`src-tauri/src/lib.rs` 新增命令注册会触发极慢编译和 Cargo cache 锁争用，需要单独处理

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
