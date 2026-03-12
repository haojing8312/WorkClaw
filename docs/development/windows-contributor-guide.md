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

## 4. English Summary

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
