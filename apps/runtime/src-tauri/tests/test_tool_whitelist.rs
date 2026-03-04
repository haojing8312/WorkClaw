use runtime_lib::agent::ToolRegistry;

#[test]
fn test_get_filtered_definitions() {
    let registry = ToolRegistry::with_file_tools();

    let all = registry.get_tool_definitions();
    assert!(all.len() >= 6); // read_file, write_file, glob, grep, edit, todo_write

    let whitelist = vec!["read_file".to_string(), "glob".to_string()];
    let filtered = registry.get_filtered_tool_definitions(&whitelist);
    assert_eq!(filtered.len(), 2);
    let names: Vec<&str> = filtered.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"glob"));
    assert!(!names.contains(&"write_file"));
}

#[test]
fn test_empty_whitelist() {
    let registry = ToolRegistry::with_file_tools();
    let filtered = registry.get_filtered_tool_definitions(&[]);
    assert_eq!(filtered.len(), 0);
}

#[test]
fn test_nonexistent_whitelist_tool() {
    let registry = ToolRegistry::with_file_tools();
    let whitelist = vec!["nonexistent_tool".to_string()];
    let filtered = registry.get_filtered_tool_definitions(&whitelist);
    assert_eq!(filtered.len(), 0);
}
