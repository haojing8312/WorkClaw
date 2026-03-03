# Milestone 1 收尾功能设计

> **Date**: 2026-02-26
> **Status**: Approved
> **Author**: Claude Sonnet 4.6

---

## 模块 1: File Upload 支持

### 目标

允许用户上传文件作为对话上下文，文件内容将发送给 LLM。

### 参考实现

参考 Open Claude Cowork 的实现方式。

### UI/UX 设计

**附件按钮**：
- 位置：ChatView 输入区工具栏（发送按钮左侧）
- 图标：📎（回形针）
- 样式：与现有工具栏按钮一致

**文件选择**：
- 触发：点击附件按钮 → 打开系统文件选择器
- 多选：支持 `multiple` 属性
- 限制：最多 5 个文件

**附件列表展示**：
- 位置：输入框上方
- 显示：文件名 + 文件大小 + 删除按钮
- 样式：浅色背景药丸标签

**大文件处理**：
- 限制：单文件 ≤ 5MB
- 超出提示：`alert('单个文件不能超过 5MB')`
- 读取方式：文本文件 → 文本内容，图片 → base64

### 技术实现

**前端 (ChatView.tsx)**：
```tsx
// 状态
const [attachedFiles, setAttachedFiles] = useState<FileAttachment[]>([]);

// 文件选择处理
const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
  const files = Array.from(e.target.files || []);
  // 验证数量和大小
  // 读取文件内容
  // 更新状态
};

// 发送时附加文件内容
const content = files.length > 0
  ? `${message}\n\n---\n\n附件文件：\n${files.map(f => `## ${f.name}\n\`\`\`\n${f.content}\n\`\`\``).join('\n\n')}`
  : message;
```

**消息格式**：
```json
{
  "role": "user",
  "content": "用户消息\n\n---\n\n附件文件：\n## filename.ts\n```\n文件内容\n```"
}
```

### 限制

| 限制项 | 值 |
|--------|-----|
| 最大文件数 | 5 |
| 单文件大小 | 5MB |
| 支持类型 | 所有（文本优先） |

---

## 模块 2: Secure Workspace 配置

### 目标

为每个会话配置独立的工作空间目录，Agent 只能在该目录下操作文件。

### 数据模型

**Sessions 表变更**：
```sql
ALTER TABLE sessions ADD COLUMN workspace_path TEXT;
```

**默认值**：`C:\Users\<用户名>\.workclaw\projects`

### UI/UX 设计

**工作空间显示**：
- 位置：ChatView 头部（模型名称右侧）
- 样式：点击按钮，显示当前路径
- 图标：📁 + 路径文字

**下拉列表**：
- 最近使用：显示最近 5 个工作目录
- 选择新文件夹：打开目录选择器
- 样式：下拉菜单，浅色主题

**目录选择器**：
- API：Tauri `dialog::open({ directory: true })`
- 选择后：更新当前会话的 `workspace_path`

### 权限检查

**工具执行前验证**：
```rust
fn validate_workspace(path: &Path, workspace: &Path) -> bool {
    path.starts_with(workspace)
}
```

**受限工具**：
- `read_file`
- `write_file`
- `list_dir`
- `glob`
- `grep`
- `file_stat`
- `file_delete`
- `file_move`
- `file_copy`

### 行为

1. **新建对话**：继承上一个对话的工作空间
2. **首次使用**：使用默认路径 `C:\Users\<用户名>\.workclaw\projects`
3. **目录不存在**：自动创建
4. **越权访问**：返回错误提示

---

## 模块 3: 手动触发压缩

### 目标

允许用户主动触发上下文压缩，释放 token 预算。

### 触发方式

**方式 1：按钮触发**
- 位置：ChatView 输入区工具栏
- 图标：📦（打包/压缩）
- 状态：压缩进行时显示 loading

**方式 2：命令触发**
- 命令：`/compact`
- 处理：识别命令 → 移除命令文本 → 触发压缩

### 执行流程

```rust
// 1. 估算当前 token 数
let estimated_tokens = estimate_tokens(&messages);

// 2. 保存完整记录到磁盘
let transcript_path = save_transcript(&transcript_dir, &session_id, &messages)?;

// 3. 调用 LLM 生成摘要
let compacted = auto_compact(
    &api_format,
    &base_url,
    &api_key,
    &model,
    &messages,
    &transcript_path,
).await?;

// 4. 更新会话消息
update_session_messages(&session_id, &compacted)?;

Ok(CompactionResult {
    original_tokens: estimated_tokens,
    new_tokens: estimate_tokens(&compacted),
    transcript_path,
    summary: summary_text,
})
```

### UI 展示

**压缩进行中**：
- 输入框显示："正在压缩上下文..."
- 按钮显示 loading 状态
- 禁用输入

**压缩完成后**：
- 显示 token 节省信息：`"已压缩上下文：50,000 → 12,000 tokens"`
- 显示摘要内容（作为 assistant 消息插入）

**示例展示**：
```
[用户]: /compact

[系统]: 📦 上下文已压缩：50,000 → 12,000 tokens

[助手]: ## 对话摘要
### 用户请求
用户希望实现一个文件上传功能...

### 已完成
- 分析了项目结构
- 设计了 UI 方案

### 待办
- 实现前端组件
- 添加后端 API
```

---

## 实施顺序

1. **File Upload** - 最简单，先实现
2. **Secure Workspace** - 需要数据库变更
3. **手动压缩** - 需要后端 LLM 调用

---

## 相关文件

- `apps/runtime/src/components/ChatView.tsx` - UI 修改
- `apps/runtime/src-tauri/src/db.rs` - 数据库变更
- `apps/runtime/src-tauri/src/agent/permissions.rs` - 权限检查
- `apps/runtime/src-tauri/src/agent/compactor.rs` - 压缩功能
- `apps/runtime/src-tauri/src/commands/chat.rs` - 会话管理
