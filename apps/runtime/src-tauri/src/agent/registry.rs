use super::tool_manifest::ToolManifestEntry;
use super::tools::{
    BashTool, EditTool, ExecTool, FileCopyTool, FileDeleteTool, FileMoveTool, FileStatTool,
    GlobTool, GrepTool, ListDirTool, OpenInFolderTool, ReadFileTool, ScreenshotTool, TodoWriteTool,
    WebFetchTool, WriteFileTool,
};
use super::types::Tool;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// 基础工具集：文件操作 + Shell + 信息获取 + 系统工具
    pub fn with_standard_tools() -> Self {
        let registry = Self::new();
        registry.register(Arc::new(ReadFileTool));
        registry.register(Arc::new(WriteFileTool));
        registry.register(Arc::new(GlobTool));
        registry.register(Arc::new(GrepTool));
        registry.register(Arc::new(EditTool));
        // L2 新增文件工具
        registry.register(Arc::new(ListDirTool));
        registry.register(Arc::new(FileStatTool));
        registry.register(Arc::new(FileDeleteTool));
        registry.register(Arc::new(FileMoveTool));
        registry.register(Arc::new(FileCopyTool));
        registry.register(Arc::new(TodoWriteTool::new()));
        registry.register(Arc::new(WebFetchTool));
        registry.register(Arc::new(ExecTool::new()));
        registry.register(Arc::new(BashTool::new()));
        // L5 新增系统工具
        registry.register(Arc::new(ScreenshotTool));
        registry.register(Arc::new(OpenInFolderTool));
        registry
    }

    pub fn register(&self, tool: Arc<dyn Tool>) {
        self.tools
            .write()
            .unwrap()
            .insert(tool.name().to_string(), tool);
    }

    pub fn unregister(&self, name: &str) {
        self.tools.write().unwrap().remove(name);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.read().unwrap().get(name).cloned()
    }

    pub fn tool_names(&self) -> Vec<String> {
        let mut names = self
            .tools
            .read()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    pub fn tool_manifest_entries(&self) -> Vec<ToolManifestEntry> {
        let tools = self.tools.read().unwrap();
        let mut entries = tools
            .iter()
            .map(|(name, tool)| {
                ToolManifestEntry::from_parts(name, tool.description(), tool.metadata())
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        entries
    }

    pub fn standard_tool_names() -> Vec<&'static str> {
        vec![
            "bash",
            "edit",
            "exec",
            "file_copy",
            "file_delete",
            "file_move",
            "file_stat",
            "glob",
            "grep",
            "list_dir",
            "open_in_folder",
            "read_file",
            "screenshot",
            "todo_write",
            "web_fetch",
            "write_file",
        ]
    }

    pub fn get_tool_definitions(&self) -> Vec<Value> {
        self.tools
            .read()
            .unwrap()
            .values()
            .map(|t| {
                json!({
                    "name": t.name(),
                    "description": t.description(),
                    "input_schema": t.input_schema(),
                })
            })
            .collect()
    }

    /// 返回仅包含白名单中工具的定义
    pub fn get_filtered_tool_definitions(&self, whitelist: &[String]) -> Vec<Value> {
        self.tools
            .read()
            .unwrap()
            .values()
            .filter(|t| whitelist.iter().any(|w| w == t.name()))
            .map(|t| {
                json!({
                    "name": t.name(),
                    "description": t.description(),
                    "input_schema": t.input_schema(),
                })
            })
            .collect()
    }

    /// 向后兼容别名
    pub fn with_file_tools() -> Self {
        Self::with_standard_tools()
    }

    /// 返回所有以指定前缀开头的工具名称
    pub fn tools_with_prefix(&self, prefix: &str) -> Vec<String> {
        self.tools
            .read()
            .unwrap()
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::ToolRegistry;
    use crate::agent::tool_manifest::ToolCategory;

    #[test]
    fn standard_tool_surface_matches_expected_names() {
        let registry = ToolRegistry::with_standard_tools();
        let expected = ToolRegistry::standard_tool_names()
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();

        assert_eq!(registry.tool_names(), expected);
    }

    #[test]
    fn representative_standard_tools_publish_expected_metadata() {
        let registry = ToolRegistry::with_standard_tools();

        let read_file = registry.get("read_file").expect("read_file tool");
        let write_file = registry.get("write_file").expect("write_file tool");
        let bash = registry.get("bash").expect("bash tool");

        let read_meta = read_file.metadata();
        assert_eq!(read_meta.category, ToolCategory::File);
        assert!(read_meta.read_only);
        assert!(!read_meta.destructive);

        let write_meta = write_file.metadata();
        assert_eq!(write_meta.category, ToolCategory::File);
        assert!(write_meta.destructive);
        assert!(write_meta.requires_approval);

        let bash_meta = bash.metadata();
        assert_eq!(bash_meta.category, ToolCategory::Shell);
        assert!(bash_meta.destructive);
        assert!(bash_meta.requires_approval);
    }
}
