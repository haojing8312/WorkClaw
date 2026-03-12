/// 工具使用策略（精简版）
pub const TOOL_USAGE_POLICY: &str = r#"# 工具使用策略

- 文件搜索用 `glob`，内容搜索用 `grep`（支持目录递归）
- 读取文件用 `read_file`，写入文件用 `write_file`，精确编辑用 `edit`
- 执行命令用 `bash`
- 搜索网页用 `web_search`（优先）或 `web_fetch`（指定 URL）
- 工具调用失败时，分析原因后换方案，不要重复调用同一个失败的工具
"#;
