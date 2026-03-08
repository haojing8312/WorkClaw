@echo off
setlocal

if not defined CARGO_HOME set "CARGO_HOME=%USERPROFILE%\.cargo"
if not defined RUSTUP_HOME set "RUSTUP_HOME=%USERPROFILE%\.rustup"

set "PATH=%RUSTUP_HOME%\toolchains\stable-x86_64-pc-windows-msvc\bin;%CARGO_HOME%\bin;%PATH%"
cd /d "%~dp0"
pnpm app > "%~dp0app-launch.log" 2>&1
