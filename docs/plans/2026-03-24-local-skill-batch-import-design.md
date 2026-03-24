# Local Skill Batch Import Design

## Scope

扩展专家技能安装弹窗中的“本地目录”导入能力。用户选择一个本地目录后，系统既支持导入单个 Skill 目录，也支持把类似 `.agents/skills` 的 skills 根目录批量导入。目录扫描最多只向下再找一层。

## Goals

- 保持“本地目录”作为单一入口，不新增新的安装模式。
- 允许用户选择单个 skill 目录，继续按现有体验导入。
- 允许用户选择 skill 集合根目录，自动检测并批量导入其中的多个 skill。
- 当目录中存在多个 skill 时，直接全部导入，不要求额外勾选。
- 支持部分成功：单个 skill 失败不阻断其他 skill 的导入。
- 在导入完成后向前端返回清晰的成功、失败、缺失 MCP 依赖汇总。

## Non-Goals

- 不支持无限递归扫描目录。
- 不新增批量导入预览页或勾选式选择器。
- 不改变 ClawHub、`.skillpack`、行业包 的安装流程。
- 不修改已安装 skill 的刷新逻辑。

## Recommended Approach

将目录扫描和批量导入逻辑统一收敛到 Tauri 后端，前端继续只负责选择本地目录和展示结果。

后端导入逻辑按以下规则运行：

1. 如果所选目录自身包含 `SKILL.md` 或 `skill.md`，将它视为单个 skill 目录并直接导入。
2. 否则扫描该目录的直接子目录。
3. 对每个直接子目录：
   - 如果子目录自身包含 `SKILL.md` 或 `skill.md`，将其识别为一个 skill。
   - 否则继续扫描这个子目录的直接子目录，并识别其中包含 `SKILL.md` 或 `skill.md` 的目录。
4. 不再继续更深层递归。

识别出 skill 目录列表后，后端按稳定顺序逐个导入，并收集：

- 成功导入的 skill 清单
- 失败条目及原因
- 所有成功导入 skill 的 `missing_mcp` 去重汇总

## API Design

将本地导入接口从单结果结构扩展为批量结果结构，统一兼容单目录导入与批量目录导入。

建议返回结构：

- `installed`: 成功导入的 skill 列表，元素含 `manifest`
- `failed`: 失败列表，元素含 `dir_path`、`name_hint`、`error`
- `missing_mcp`: 所有成功导入 skill 缺失的 MCP 服务名去重汇总

前端不再假设只有一个 `manifest.id`，而是读取：

- 第一个成功导入 skill 的 `id` 用于沿用现有 `onInstalled` 回调
- 成功数量和失败数量用于提示文案
- `missing_mcp` 用于现有风险提示区展示

## UX Changes

“本地目录”模式保持当前 tab 不变，但文案调整为更贴近批量能力：

- 选择按钮文案从“选择 Skill 目录”改为“选择 Skill 目录或 skills 根目录”
- 说明文案改为说明两种受支持输入：
  - 单个 Skill 目录
  - 包含多个 Skill 的根目录
- 明确写出扫描深度限制：最多向下扫描一层子目录

安装完成后的反馈策略：

- 成功 1 个且失败 0 个：保持接近当前体验
- 成功多个：提示“已导入 N 个 Skill”
- 有失败项：追加显示“失败 M 个”，并列出失败原因
- 若成功数为 0：视为失败，展示聚合错误

## Error Handling

需要覆盖以下情况：

- 所选目录不存在任何可导入的 `SKILL.md`：返回明确错误
- 某个 skill 重名导致 `DUPLICATE_SKILL_NAME`：只标记该项失败，不影响其他项
- 某个 `SKILL.md` 读取失败或 front matter 解析失败：只标记该项失败
- 全部失败：前端作为安装失败处理，不触发 `onInstalled`

## Affected Modules

- `apps/runtime/src/components/InstallDialog.tsx`
- `apps/runtime/src/components/__tests__/InstallDialog.*.test.tsx`
- `apps/runtime/src-tauri/src/commands/skills.rs`
- `apps/runtime/src-tauri/src/commands/skills/types.rs`
- `apps/runtime/src-tauri/src/commands/skills/local_skill_service.rs`
- `apps/runtime/src-tauri/tests/test_skill_commands.rs`

## Risks

- 现有前端调用方默认单个 `manifest.id`，返回结构变更需要同步调整。
- 批量导入的部分成功语义如果处理不当，可能导致安装成功但弹窗文案不清楚。
- 扫描逻辑若未去重，可能重复识别同一个目录。

## Verification

- Rust 命令测试验证目录扫描、批量导入、部分成功和深度限制。
- 前端组件测试验证“本地目录”模式的调用参数、结果提示和 `onInstalled` 行为。
- 运行最小命令集覆盖 Tauri Rust 与前端运行时表层。
