# Windows Contributor Guide

面向从源码运行 WorkClaw 桌面应用的贡献者，以及负责 Windows 自动发布的维护者。

## 1. Windows 贡献者前置要求（源码运行）

如果你只是想使用 WorkClaw，请优先下载 Release 安装包。下面这些要求是给从源码运行桌面应用的贡献者准备的。

- Windows 10 / 11 x64
- Rust stable + `x86_64-pc-windows-msvc`
- Visual Studio 2022 Build Tools（稳定版）
- `Desktop development with C++`
- Windows 10/11 SDK
- WebView2 Runtime

如果本地构建失败，先运行：

```bash
pnpm doctor:windows
```

常见 Windows 本地构建问题请看：

- [docs/troubleshooting/windows-dev-setup.md](/e:/code/yzpd/workclaw/docs/troubleshooting/windows-dev-setup.md)

## 2. 本地快速启动 Tauri 窗口

```bash
# 1) 仅首次或依赖变更后执行
pnpm install

# Windows 源码构建失败时，先做本机环境诊断
pnpm doctor:windows

# 2) 若报错 "Port 5174 is already in use"，先定位并结束占用进程
netstat -ano | findstr LISTENING | findstr :5174
taskkill /PID <PID> /F

# 3) 从仓库根目录启动 Tauri 桌面窗口
pnpm app
```

启动成功后可用下面两条命令快速自检：

```bash
# 前端开发服务已启动（应返回 HTTP 200）
curl -I http://localhost:5174

# Tauri 桌面进程已启动（应看到 runtime.exe）
tasklist | findstr /I runtime.exe
```

退出测试（按需）：

```bash
# 先结束 5174 端口监听进程
netstat -ano | findstr LISTENING | findstr :5174
taskkill /PID <PID> /F

# 再结束 runtime.exe 对应 PID（只杀你本次测试启动的 PID）
tasklist | findstr /I runtime.exe
taskkill /PID <RUNTIME_PID> /F
```

## 3. Windows 自动 Release（GitHub）

已支持 `tag` 自动发布 Windows 安装包到 GitHub Release。

```bash
# 1) 确保版本与 tag 一致（apps/runtime/src-tauri/tauri.conf.json -> version）
# 2) 推送语义化 tag（触发 .github/workflows/release-windows.yml）
git tag v0.1.0
git push origin v0.1.0
```

发布前会执行版本一致性校验：`tag(vX.Y.Z)` 必须与 `tauri.conf.json` 的 `version` 相同。

发布产物使用建议：

- `*-setup.exe`：推荐普通用户下载，适合直接安装使用。
- `*.msi`：适合企业 IT、批量部署和手动升级。

如果你只是想安装并直接使用 WorkClaw，请优先选择 `.exe` 安装包。

## 4. 本地构建缓存治理

仓库现在会在 `pnpm install` 时自动把本地 git hooks 安装到 `.githooks`，并在 `git commit` / `git push` 前运行构建缓存治理脚本。

默认策略：

- 自动清理 `cargo-targets/workclaw/debug/incremental`
  - 最后修改时间超过 `7` 天
  - 或目录大小超过 `20 GB`
- 自动清理 `.cargo-targets/isolated`
  - 单个隔离构建目录最后修改时间超过 `3` 天
  - 或总量超过 `20 GB`
  - 或只保留最近 `5` 个隔离构建目录
- 只校验 `cargo-targets/workclaw/debug/deps`
  - 目录大小超过 `40 GB` 时阻止提交/推送
  - 不会自动删除，避免破坏当前可复用构建产物

手动命令：

```bash
# 只做检查（本地会按策略自动清 incremental）
pnpm cache:build:check

# 手动深度清理 incremental + deps（先停止 cargo / pnpm app）
pnpm cache:build:clean -- --include-deps
```

CI 也会运行同一个脚本，但使用只读模式，不会在 runner 上删除文件。

## 5. Windows 下员工智能体重型回归

在部分 Windows 本地环境里，`apps/runtime/src-tauri/tests/test_im_employee_agents.rs` 的重型场景族会在 Rust `libtest` 进程启动前触发历史性的 `0xc0000139` 问题。为了避免这个历史环境问题阻塞开发，当前 Windows 回归分成两层：

- 轻量 `libtest` 回归：
  - `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents -- --list`
  - 这里保留轻量 employee-id / 基础契约场景，确保本地 `cargo test` 仍然可用
- 重型普通二进制回归：
  - `pnpm test:group-run-regression`
  - `pnpm test:employee-im-heavy-regression`
  - 这些命令覆盖 group run、IM routing、team entry、group management 等重型场景

统一入口：

```bash
pnpm test:employee-agents-windows
```

这个命令会按顺序执行：

- `test_im_employee_agents` 轻量场景列举
- `employee_group_run_regression`
- `employee_im_heavy_regression`

如果你在 Windows 本机看到 `0xc0000139`，优先使用上面的统一入口，而不要把它当成当前业务逻辑回归失败。

## 6. English Summary

This document covers the Windows-specific contributor path for source builds and the GitHub-based Windows release flow.

### Prerequisites

- Windows 10 / 11 x64
- Rust stable with `x86_64-pc-windows-msvc`
- Visual Studio 2022 Build Tools
- `Desktop development with C++`
- Windows 10/11 SDK
- WebView2 Runtime

If a local build fails, run `pnpm doctor:windows` first.

### Local Run

- Install dependencies: `pnpm install`
- Run diagnostics when needed: `pnpm doctor:windows`
- Free port `5174` if occupied
- Start the desktop app from repo root: `pnpm app`

Quick verification:

- `curl -I http://localhost:5174`
- `tasklist | findstr /I runtime.exe`

### Auto Release

Windows release is tag-driven through `.github/workflows/release-windows.yml`.

```bash
git tag v0.1.0
git push origin v0.1.0
```

CI validates that the pushed `tag(vX.Y.Z)` matches `apps/runtime/src-tauri/tauri.conf.json` `version`.

### Build Cache Governance

The repo now installs git hooks from `.githooks` during `pnpm install` and runs a shared cache governance script before `git commit` and `git push`.

- `cargo-targets/workclaw/debug/incremental`
  - auto-pruned when older than `7` days
  - or when larger than `20 GB`
- `.cargo-targets/isolated`
  - auto-pruned when individual isolated runs are older than `3` days
  - or when the total isolated cache is larger than `20 GB`
  - or when more than `5` isolated run directories are present
- `cargo-targets/workclaw/debug/deps`
  - checked against a `40 GB` limit
  - never auto-deleted by hooks because it may still be an active reusable build output

Manual commands:

- `pnpm cache:build:check`
- `pnpm cache:build:clean -- --include-deps`

### Heavy Employee-Agent Regressions On Windows

Some local Windows environments can hit a historical pre-main `0xc0000139` failure when the heavyweight `test_im_employee_agents` integration binary is launched by Rust `libtest`.

The Windows regression path is intentionally split:

- lightweight `libtest` coverage:
  - `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents -- --list`
- heavyweight regression binaries:
  - `pnpm test:group-run-regression`
  - `pnpm test:employee-im-heavy-regression`

Recommended single entrypoint on Windows:

```bash
pnpm test:employee-agents-windows
```

Use this command as the canonical local regression path for employee-agent behavior on Windows when the historical `0xc0000139` issue is present.
