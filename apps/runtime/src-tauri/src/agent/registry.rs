use super::tools::{
    BashTool, EditTool, FileCopyTool, FileDeleteTool, FileMoveTool, FileStatTool, GlobTool,
    GrepTool, ListDirTool, OpenInFolderTool, ReadFileTool, ScreenshotTool, TodoWriteTool,
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
