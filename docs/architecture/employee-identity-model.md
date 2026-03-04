# 员工身份模型（employee_id）

本文档说明 WorkClaw 当前的员工身份字段设计与兼容策略。

## 统一字段

- 对外统一使用单字段：`employee_id`（员工编号）
- 前端界面仅暴露 `employee_id`，减少概念复杂度

## 兼容映射策略

- 保存时自动镜像：`role_id = employee_id`
- 保存时自动镜像：`openclaw_agent_id = employee_id`
- 数据迁移回填：当 `employee_id` 为空时，自动使用历史 `role_id`

## 配置向导产物

- 员工页支持问答式生成并预览：`AGENTS.md` / `SOUL.md` / `USER.md`
- 一键应用后写入目录：`<employee_work_dir>/openclaw/<employee_id>/`

## 相关文档

- 技能安装排错：`docs/troubleshooting/skill-installation.md`
