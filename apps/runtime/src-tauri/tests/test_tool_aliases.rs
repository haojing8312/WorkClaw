use runtime_lib::agent::tools::{
    register_tool_alias, BashTool, GlobTool, ListDirTool, ReadFileTool,
};
use runtime_lib::agent::ToolRegistry;
use std::sync::Arc;

#[test]
fn test_openclaw_style_aliases_delegate_to_existing_tools() {
    let registry = ToolRegistry::new();
    let read = Arc::new(ReadFileTool);
    let find = Arc::new(GlobTool);
    let ls = Arc::new(ListDirTool);
    let exec = Arc::new(BashTool::new());

    registry.register(read.clone());
    registry.register(find.clone());
    registry.register(ls.clone());
    registry.register(exec.clone());

    register_tool_alias(&registry, "read", read);
    register_tool_alias(&registry, "find", find);
    register_tool_alias(&registry, "ls", ls);
    register_tool_alias(&registry, "exec", exec);

    assert!(registry.get("read").is_some());
    assert!(registry.get("find").is_some());
    assert!(registry.get("ls").is_some());
    assert!(registry.get("exec").is_some());
}
