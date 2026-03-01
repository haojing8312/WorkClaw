# 2026-03-01 Expert Skills Hub Acceptance

## Scope
- 创建技能目录冲突保护
- 创建成功但导入失败的兜底与重试
- 专家技能基础管理（刷新/移除）
- 内置 skill metadata 启动同步

## Acceptance Checklist
1. 创建同名技能时返回冲突错误，不覆盖已有目录  
Status: Pass

2. 技能文件已保存但导入失败时，界面显示保存路径并允许重试导入  
Status: Pass

3. 专家技能页可刷新本地技能，可移除非内置技能  
Status: Pass

4. 启动时内置 skill 使用 upsert 同步最新 metadata（非 insert-ignore）  
Status: Pass

## Evidence
- Frontend tests:
  - `npm test` -> 23 passed
  - Coverage includes:
    - `src/__tests__/App.experts-routing.test.tsx`
    - `src/components/experts/__tests__/ExpertsView.test.tsx`

- Frontend build:
  - `npm run build` -> passed

- Rust checks:
  - `cargo check` -> passed
  - `cargo test --test test_skill_commands` -> 3 passed
  - `cargo test sync_builtin_general_skill --lib` -> 2 passed

## Notes
- 当前环境无法执行 GUI 级手工验收（Tauri 桌面窗口交互），已通过自动化测试与构建验证主要发布风险点。
