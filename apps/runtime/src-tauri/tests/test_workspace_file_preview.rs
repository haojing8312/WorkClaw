use runtime_lib::commands::workspace_files::{
    list_workspace_files_within, read_workspace_file_preview_within,
};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use zip::write::FileOptions;
use zip::ZipWriter;

fn write_docx(path: &PathBuf, text: &str) {
    let file = fs::File::create(path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default();
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"></Types>"#)
        .unwrap();
    zip.add_directory("word/", options).unwrap();
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:body>
                <w:p><w:r><w:t>{}</w:t></w:r></w:p>
              </w:body>
            </w:document>"#,
            text
        )
        .as_bytes(),
    )
    .unwrap();
    zip.finish().unwrap();
}

fn make_temp_workspace(name: &str) -> PathBuf {
    let root = PathBuf::from(format!("{}_tmp", name));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(root.join(".minimax")).unwrap();
    fs::write(root.join("conflict_brief.md"), "# Brief\n\nHello").unwrap();
    fs::write(
        root.join("conflict_report.html"),
        "<html><body>Report</body></html>",
    )
    .unwrap();
    write_docx(
        &root.join("conflict_brief.docx"),
        "美国以色列伊朗冲突 Word 简报",
    );
    fs::write(root.join("large_preview.md"), "A".repeat(300 * 1024)).unwrap();
    fs::write(root.join("archive.bin"), vec![0u8, 159, 146, 150]).unwrap();
    root
}

#[test]
fn lists_workspace_files_recursively_with_stable_sorting() {
    let root = make_temp_workspace("test_workspace_files_list");
    let entries =
        list_workspace_files_within(root.to_str().unwrap()).expect("list workspace files");

    assert!(entries
        .iter()
        .any(|item| item.path == ".minimax" && item.kind == "directory"));
    assert!(entries
        .iter()
        .any(|item| item.path == "conflict_brief.md" && item.kind == "markdown"));
    assert!(entries
        .iter()
        .any(|item| item.path == "conflict_report.html" && item.kind == "html"));
    assert!(entries
        .iter()
        .any(|item| item.path == "conflict_brief.docx" && item.kind == "docx"));
    assert!(entries
        .iter()
        .any(|item| item.path == "archive.bin" && item.kind == "binary"));
    assert!(entries
        .iter()
        .any(|item| item.path == "large_preview.md" && item.kind == "markdown"));

    let names: Vec<_> = entries.iter().map(|item| item.path.clone()).collect();
    assert_eq!(
        names,
        vec![
            ".minimax".to_string(),
            "archive.bin".to_string(),
            "conflict_brief.docx".to_string(),
            "conflict_brief.md".to_string(),
            "conflict_report.html".to_string(),
            "large_preview.md".to_string(),
        ]
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn returns_text_preview_for_markdown_html_and_docx_files() {
    let root = make_temp_workspace("test_workspace_files_preview_text");

    let markdown = read_workspace_file_preview_within(root.to_str().unwrap(), "conflict_brief.md")
        .expect("markdown preview");
    assert_eq!(markdown.kind, "markdown");
    assert!(markdown.source.unwrap().contains("# Brief"));
    assert!(!markdown.truncated);
    assert!(markdown.preview_error.is_none());

    let html = read_workspace_file_preview_within(root.to_str().unwrap(), "conflict_report.html")
        .expect("html preview");
    assert_eq!(html.kind, "html");
    assert!(html.source.unwrap().contains("<html>"));
    assert!(!html.truncated);
    assert!(html.preview_error.is_none());

    let docx = read_workspace_file_preview_within(root.to_str().unwrap(), "conflict_brief.docx")
        .expect("docx preview");
    assert_eq!(docx.kind, "docx");
    assert!(docx
        .source
        .unwrap()
        .contains("美国以色列伊朗冲突 Word 简报"));
    assert!(!docx.truncated);
    assert!(docx.preview_error.is_none());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn marks_large_text_previews_as_truncated() {
    let root = make_temp_workspace("test_workspace_files_preview_truncated");

    let preview = read_workspace_file_preview_within(root.to_str().unwrap(), "large_preview.md")
        .expect("large markdown preview");
    assert_eq!(preview.kind, "markdown");
    assert!(preview.truncated);
    assert_eq!(preview.source.unwrap().len(), 256 * 1024);
    assert!(preview.preview_error.is_none());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn returns_metadata_only_for_binary_files() {
    let root = make_temp_workspace("test_workspace_files_preview_binary");

    let preview = read_workspace_file_preview_within(root.to_str().unwrap(), "archive.bin")
        .expect("binary preview");
    assert_eq!(preview.kind, "binary");
    assert!(preview.source.is_none());
    assert!(preview.size > 0);
    assert!(!preview.truncated);
    assert!(preview.preview_error.is_none());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_paths_outside_workspace() {
    let root = make_temp_workspace("test_workspace_files_preview_outside");

    let result = read_workspace_file_preview_within(root.to_str().unwrap(), "../secret.txt");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("不允许越出工作空间"));

    fs::remove_dir_all(root).unwrap();
}
