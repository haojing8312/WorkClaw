/// 工具使用策略（精简版）
pub const TOOL_USAGE_POLICY: &str = r#"# 工具使用策略

- 文件搜索用 `glob`，内容搜索用 `grep`（支持目录递归）
- 读取文件用 `read_file`，写入文件用 `write_file`，精确编辑用 `edit`
- 执行命令用 `exec`；Windows 按 PowerShell 语法，Unix 按 bash 语法
- 只有明确需要兼容旧 shell 行为时才用 `bash`
- 长时间命令优先设置合适的 `timeout_ms`，不需要立即等结果时使用 `background: true`
- `exec` 后台进程用 `exec_output` 查看输出、用 `exec_kill` 终止
- 搜索网页用 `web_search`（优先）或 `web_fetch`（指定 URL）
- 工具调用失败时，分析原因后换方案，不要重复调用同一个失败的工具
"#;
