# Windows 开发环境排障

这份文档面向 **从源码运行 WorkClaw 的贡献者**，不是面向普通安装用户。

如果你只是想使用应用，请优先下载 [GitHub Releases](https://github.com/haojing8312/WorkClaw/releases) 中的安装包，而不是本地编译源码。

## 支持基线

当前仓库对 Windows 本地源码构建的支持基线是：

- Windows 10 / 11 x64
- Node.js 20+
- pnpm
- Rust stable
- Rust target: `x86_64-pc-windows-msvc`
- Visual Studio 2022 Build Tools（稳定版）
- Visual Studio workload: `Desktop development with C++`
- Windows 10/11 SDK
- WebView2 Runtime

说明：

- `Visual Studio Preview / Insiders` 目前只按 best effort 处理，不作为默认支持基线。
- `cargo` 报某个 crate 编译失败，并不代表那个 crate 本身有问题；很多 Windows 构建失败实际发生在链接阶段。

## 快速诊断

在仓库根目录运行：

```bash
pnpm doctor:windows
```

如果你已经拿到了构建日志，也可以把日志文件传给 doctor，帮助识别典型链接错误：

```bash
pnpm doctor:windows --error-file path\\to\\build.log
```

同时建议补充这些命令输出：

```bash
node -v
pnpm -v
rustc -vV
rustup show
where link
```

## 常见问题

| 症状 | 常见原因 | 处理方式 |
| --- | --- | --- |
| `LINK : fatal error LNK1104: cannot open file 'msvcrt.lib'` | MSVC 工具链不完整、缺少 `Desktop development with C++`、缺少 Windows SDK，或 shell 没刷新 | 在 Visual Studio Installer 中安装稳定版 Visual Studio 2022 Build Tools + `Desktop development with C++` + Windows SDK，然后重新打开终端 |
| `link.exe` not found | 没装 C++ Build Tools，或当前 shell 没拿到编译链 | 安装稳定版 Visual Studio 2022 Build Tools，并重新打开终端 |
| `target x86_64-pc-windows-msvc not found` | Rust Windows MSVC target 没装 | 运行 `rustup target add x86_64-pc-windows-msvc` |
| `Port 5174 is already in use` | 前端开发端口被旧进程占用 | 用 `netstat -ano | findstr :5174` 找到 PID，再只结束对应 PID |
| Tauri 窗口白屏或无法启动 | WebView2 缺失、前端 dev server 没启动，或本地原生环境不完整 | 先检查 `curl -I http://localhost:5174`，再检查 WebView2 和 `pnpm doctor:windows` 输出 |

## 推荐排查顺序

1. 先确认自己是在“从源码构建”的场景，不是普通安装场景。
2. 运行 `pnpm doctor:windows`。
3. 如果 doctor 已经指出缺少 `link.exe`、MSVC 或 Windows SDK，先修环境，不要先怀疑业务代码。
4. 如果报错包含 `msvcrt.lib`，优先检查 Visual Studio Build Tools 和 Windows SDK。
5. 如果 doctor 输出正常，但问题仍然稳定复现，再提交 issue。

## 提交 issue 时请附带

- 操作系统版本和架构
- 失败命令
- 错误日志摘录
- `rustc -vV`
- `rustup show`
- `where link`
- 是否使用 Visual Studio 稳定版还是 Preview / Insiders
- `pnpm doctor:windows` 输出

这样维护者可以更快判断是本机环境问题还是仓库缺陷。
