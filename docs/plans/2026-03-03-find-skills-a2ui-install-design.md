# 找技能 A2UI 安装确认设计

## 背景
当前“找技能”能力已能在会话中检索 ClawHub 候选技能，但用户在安装时需要切换到其他入口，路径割裂。  
目标是在“找技能”对话上下文内直接提供安装动作，并通过 A2UI 弹窗做二次确认。

## 目标
- 在 ChatView 中展示“找技能”返回的可安装候选卡片。
- 用户点击“立即安装”后弹出 A2UI 确认框。
- 确认后直接调用 `install_clawhub_skill` 完成安装，并刷新技能列表状态。

## 非目标
- 不改造 ClawHub 服务端接口。
- 不新增复杂权限体系（复用现有确认弹窗形态即可）。
- 不实现批量安装，仅支持单技能逐个安装。

## 用户流程
1. 用户在“找技能”会话描述需求。
2. Agent 调用 `clawhub_recommend` / `clawhub_search`，返回结构化 `items`。
3. ChatView 解析该工具输出并渲染候选技能卡片。
4. 用户点击“立即安装”。
5. 弹出 A2UI 安装确认对话框（取消/确认安装）。
6. 确认后调用 `install_clawhub_skill`。
7. 安装成功后提示结果并刷新全局技能列表；失败则展示错误并允许重试。

## 架构设计

### 1) 数据来源与解析
- 仅从 assistant 消息 `streamItems` 中提取 `tool_call`：
  - `toolCall.name in {"clawhub_recommend", "clawhub_search"}`
  - `toolCall.output` 为 JSON，且 `source = "clawhub"`，包含 `items`。
- 解析后规范化字段：
  - `slug`（必填）
  - `name`（必填）
  - `description`（可选）
  - `stars`（可选）
  - `github_url/source_url`（可选）
- 去重策略：按 `slug` 去重；优先保留信息更完整（描述更长、stars 更高）的条目。

### 2) 前端展示与交互
- 在 `ChatView` 的消息渲染区域增加 `SkillInstallCandidates` 区块。
- 每条候选卡片显示：
  - 名称、slug、描述、stars
  - `立即安装` 按钮
- 已安装态识别：
  - 若 `skills` 集合已包含 `clawhub-{slug}`，按钮显示 `已安装` 并禁用。

### 3) A2UI 安装确认弹窗
- 新增本地状态：
  - `pendingInstallSkill`
  - `showInstallConfirm`
  - `installingSlug`
  - `installError`
- 点击 `立即安装` -> 记录目标 skill 并打开确认弹窗。
- 确认动作：
  - 调用 `invoke("install_clawhub_skill", { slug, githubUrl })`
  - loading 期间禁用重复提交
- 取消动作：
  - 关闭弹窗，保留卡片与上下文。

### 4) 与 App 的状态协同
- `ChatView` 新增回调：`onSkillInstalled?: (skillId: string) => Promise<void> | void`
- 安装成功后，`ChatView` 触发回调，`App` 负责：
  - 刷新技能列表
  - 选择/同步当前技能状态（沿用现有逻辑）

## 错误与边界策略
- `items=[]`：不展示候选卡片，不弹窗。
- 安装接口失败：
  - 在卡片区域显示错误消息（非中断）
  - 保留“立即安装”按钮可重试。
- 工具输出解析失败：
  - 静默忽略该条输出，避免污染聊天主流程。

## 可观测性
- 前端埋点/日志（本地调试）：
  - `find_skills_candidates_rendered`
  - `find_skills_install_confirm_opened`
  - `find_skills_install_submitted`
  - `find_skills_install_succeeded/failed`

## 测试设计

### 单元/组件测试（前端）
- `ChatView` 从 `streamItems` 解析候选并渲染卡片。
- 点击“立即安装”可打开确认弹窗。
- 取消按钮可关闭弹窗。
- 确认按钮调用 `install_clawhub_skill`，参数正确。
- 安装成功触发 `onSkillInstalled`。
- 安装失败显示错误并可重试。
- 重复 slug 去重正确。
- 已安装技能按钮禁用并显示“已安装”。

### 回归验证
- 会话路径：找技能 -> 出候选 -> 点击安装 -> 弹窗确认 -> 成功安装。
- 安装完成后专家技能列表可见新技能且状态一致。

## 风险与缓解
- 风险：模型输出格式漂移影响解析。
  - 缓解：仅依赖工具结构化输出，不依赖自然语言文本格式。
- 风险：安装接口慢导致重复点击。
  - 缓解：按钮 loading + 幂等禁点。

## 验收标准
- 用户无需离开“找技能”会话即可完成技能安装。
- 弹窗确认链路稳定，取消/确认行为符合预期。
- 安装成功后列表状态与会话提示一致。
