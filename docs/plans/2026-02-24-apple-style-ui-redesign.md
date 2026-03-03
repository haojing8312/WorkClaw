# WorkClaw Runtime UI 重设计 — Apple 风格浅色清新

**日期**: 2026-02-24
**状态**: 已确认，待实施

## 背景

苹果设计师对当前 UI 的反馈核心：界面"AI 味十足"，像功能仪表盘而非让人愉悦的生产力工具。技术细节暴露过多（工具调用日志、迭代次数、JSON 结构），缺少直觉感和高级感。

## 设计决策

- **实施路径**: 方案 B — Tailwind CSS + framer-motion
- **配色方向**: 浅色清新（白底、柔和阴影，接近 Apple Notes / Notion 风格）
- **工具调用展示**: 混合模式（运行中显示动态岛，完成后自动折叠为摘要胶囊）
- **新增依赖**: `framer-motion`（~30KB gzipped）

## 模块 1：配色系统

### 色板对照

| 元素 | 当前 (深色) | 新方案 (浅色) |
|------|------------|--------------|
| 页面背景 | `bg-slate-900` | `bg-gray-50` |
| 侧边栏 | `bg-slate-800` | `bg-white` + `border-r border-gray-200` |
| 消息区 | `bg-slate-900` | `bg-gray-50` |
| 用户消息气泡 | `bg-blue-600 text-white` | `bg-blue-500 text-white` |
| 助手消息气泡 | `bg-slate-700 text-slate-100` | `bg-white text-gray-800 shadow-sm` |
| 输入区 | `bg-slate-800 border-slate-700` | `bg-white border-gray-200 shadow-sm` |
| 代码块 | `oneDark` | 保持深色高亮（浅色页面中深色代码块是标准做法） |
| 文字主色 | `text-slate-100` | `text-gray-800` |
| 辅助文字 | `text-slate-400` | `text-gray-500` |
| 强调色 | `blue-600` | `blue-500`（Apple 蓝） |

### 磨砂玻璃效果

三处关键应用：
- Agent 状态栏（sticky）：`bg-white/80 backdrop-blur-lg`
- 动态岛：`bg-white/90 backdrop-blur-md shadow-lg`
- 头部栏：`bg-white/70 backdrop-blur-sm`

## 模块 2：动态岛 — 工具调用展示

用 `ToolIsland` 组件替换当前 `ToolCallCard` 列表。

### 三种状态

**运行中** — 紧凑胶囊：
```
┌─────────────────────────────────────┐
│  ◉ 正在读取文件...         2/5 ▸   │
│  ━━━━━━━━━━━━━━━░░░░░░░            │
└─────────────────────────────────────┘
```
- 左侧脉冲蓝色圆点
- 人性化描述（"正在读取文件" 而非 `read_file`）
- 右侧进度计数 + 展开箭头
- 底部细进度条

**完成折叠** — 摘要胶囊：
```
┌──────────────────────────────┐
│  ✓ 已执行 3 个操作      ▸   │
└──────────────────────────────┘
```
- 绿色对勾，spring 动画从运行态缩小

**展开详情** — 完整列表：
```
┌──────────────────────────────────────┐
│  ✓ 已执行 3 个操作              ▾   │
├──────────────────────────────────────┤
│  ✓ read_file    src/app.rs     12ms │
│  ✓ grep         "fn main"      8ms │
│  ✓ bash         cargo test    1.2s  │
└──────────────────────────────────────┘
```
- 点击单行可二级展开看完整输入/输出

### 工具名人性化映射

| 技术名 | 显示名 |
|--------|--------|
| `read_file` | 正在读取文件 |
| `write_file` | 正在写入文件 |
| `edit` | 正在编辑文件 |
| `glob` | 正在搜索文件 |
| `grep` | 正在搜索内容 |
| `bash` | 正在执行命令 |
| `web_search` | 正在搜索网页 |
| `web_fetch` | 正在获取网页 |
| `task` | 子任务执行中 |
| `todo_write` | 正在更新任务 |
| `memory` | 正在访问记忆 |
| `ask_user` | 等待用户回复 |
| `compact` | 正在压缩上下文 |

## 模块 3：动画系统

依赖 `framer-motion`，四类动效：

### 3.1 消息出现

```
用户消息：   x: +20, opacity: 0 → x: 0, opacity: 1  (从右滑入)
助手消息：   x: -20, opacity: 0 → x: 0, opacity: 1  (从左滑入)
过渡：       spring(stiffness: 300, damping: 24), 0.3s
仅最新消息有入场动画，历史消息静态渲染。
```

### 3.2 动态岛状态切换

```
运行中胶囊：  width: 340px, height: 56px
完成摘要：    width: 260px, height: 40px   (spring 缩小)
展开详情：    width: 340px, height: auto    (spring 展开)
```
- `layout="position"` 连续变形
- 进度条 `motion.div width` 0% → 100%

### 3.3 侧边栏交互

- 列表项 hover：`scale: 1.01` + 背景色渐变
- 选中切换：背景色 spring 过渡
- 删除退出：`AnimatePresence` exit → `opacity: 0, x: -20, height: 0`

### 3.4 全局过渡

- 视图切换（聊天 ↔ 设置）：`opacity` 淡入淡出 0.2s
- 输入框获焦：`border-color` + 微妙 `shadow` 过渡
- 按钮 hover/press：`scale: 0.97` 按压感

### 性能原则

- 所有动画仅用 `transform` 和 `opacity`（GPU 加速）
- `layout` 动画限制在动态岛组件内
- 长列表历史消息不加入场动画

## 模块 4：布局与排版

### 4.1 间距

| 元素 | 当前 | 新方案 |
|------|------|--------|
| 消息气泡内边距 | `px-4 py-2` | `px-5 py-3` |
| 消息间距 | `gap-2` | `gap-4` |
| 侧边栏内边距 | `px-2` | `px-3` |
| 侧边栏列表项 | `px-2 py-1` | `px-3 py-2.5` |
| 输入区内边距 | `p-3` | `p-4` |
| 头部栏高度 | ~40px | ~52px |

### 4.2 圆角

| 元素 | 当前 | 新方案 |
|------|------|--------|
| 消息气泡 | `rounded-lg` (8px) | `rounded-2xl` (16px) |
| 输入框 | `rounded` (4px) | `rounded-xl` (12px) |
| 动态岛 | — | `rounded-2xl` (16px) |
| 按钮 | `rounded` | `rounded-lg` (8px) |
| 代码块 | `rounded-md` | `rounded-xl` (12px) |

### 4.3 阴影体系

```
shadow-sm:    助手消息气泡、侧边栏列表项 hover
shadow:       输入框
shadow-md:    动态岛
shadow-lg:    动态岛展开态、弹窗
```

### 4.4 头部栏简化

当前显示 Skill 名 + 版本 + 工作目录 + 模型名，信息过多。

```
┌──────────────────────────────────────────────┐
│  SkillName                        模型名  ⚙  │
└──────────────────────────────────────────────┘
```
- 左侧：仅 Skill 名称（加粗 `text-gray-900`）
- 右侧：模型名（`text-gray-400` 小字）+ 设置图标
- 版本、工作目录移除

### 4.5 输入区重设计

```
┌──────────────────────────────────────────────┐
│                                              │
│  ┌──────────────────────────────────────┐    │
│  │ 输入消息...                    ↑     │    │
│  └──────────────────────────────────────┘    │
│                                              │
└──────────────────────────────────────────────┘
```
- 输入框独立，带 `shadow` 和 `rounded-xl`
- 发送按钮内嵌在输入框右侧
- 停止按钮同位置替换发送按钮

## 涉及文件

| 文件 | 改动 |
|------|------|
| `package.json` | 新增 `framer-motion` 依赖 |
| `index.css` | 全局基础样式调整（背景色、字体颜色） |
| `App.tsx` | 根布局配色切换、视图切换动画 |
| `ChatView.tsx` | 消息区配色、动效、输入区重设计、动态岛集成 |
| `ToolCallCard.tsx` → `ToolIsland.tsx` | 全新组件，替换旧工具卡片 |
| `Sidebar.tsx` | 配色切换、列表项动效 |
| `SettingsView.tsx` | 配色切换 |
| `InstallDialog.tsx` | 配色切换 |

## 不变的部分

- 后端 Rust 代码：零改动
- 数据结构 `types.ts`：零改动
- 功能逻辑：所有 Tauri invoke 调用、事件监听不变
- 代码高亮：保持 `oneDark` 深色风格
