mod helpers;

use base64::Engine;
use runtime_lib::commands::chat::{
    SendMessagePart, SendMessageRequest, normalize_send_message_parts_with_pool,
};
use runtime_lib::commands::chat_attachment_policy::default_attachment_policy;
use runtime_lib::commands::chat_attachment_resolution::resolve_attachment_input;
use runtime_lib::commands::chat_attachment_validation::validate_attachment_input;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;

fn probe_ffmpeg_command(command: &PathBuf) -> bool {
    Command::new(command)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn resolve_ffmpeg_command() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for key in ["WORKCLAW_FFMPEG_PATH", "FFMPEG_PATH"] {
        if let Some(value) = std::env::var_os(key).filter(|value| !value.is_empty()) {
            candidates.push(PathBuf::from(value));
        }
    }
    candidates.push(PathBuf::from("ffmpeg"));
    candidates.push(PathBuf::from("ffmpeg.exe"));

    #[cfg(target_os = "windows")]
    {
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            let base = PathBuf::from(program_files);
            candidates.push(base.join("ffmpeg").join("bin").join("ffmpeg.exe"));
            candidates.push(base.join("FFmpeg").join("bin").join("ffmpeg.exe"));
        }
        if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
            let base = PathBuf::from(program_files_x86);
            candidates.push(base.join("ffmpeg").join("bin").join("ffmpeg.exe"));
            candidates.push(base.join("FFmpeg").join("bin").join("ffmpeg.exe"));
        }
        if let Some(chocolatey) = std::env::var_os("ChocolateyInstall") {
            candidates.push(PathBuf::from(chocolatey).join("bin").join("ffmpeg.exe"));
        }
        if let Some(local_app_data) = std::env::var_os("LocalAppData") {
            let base = PathBuf::from(local_app_data);
            candidates.push(
                base.join("Microsoft")
                    .join("WinGet")
                    .join("Links")
                    .join("ffmpeg.exe"),
            );
            candidates.push(
                base.join("Programs")
                    .join("ffmpeg")
                    .join("bin")
                    .join("ffmpeg.exe"),
            );
        }
        if let Some(user_profile) = std::env::var_os("USERPROFILE") {
            candidates.push(
                PathBuf::from(user_profile)
                    .join("scoop")
                    .join("shims")
                    .join("ffmpeg.exe"),
            );
        }
    }

    candidates.into_iter().find(probe_ffmpeg_command)
}

fn build_minimal_pdf_with_text(text: &str) -> Vec<u8> {
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    let objects = [
        "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_string(),
        "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_string(),
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 144] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n".to_string(),
        {
            let escaped = text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
            let stream = format!("BT\n/F1 24 Tf\n72 72 Td\n({escaped}) Tj\nET");
            format!(
                "4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n",
                stream.len(),
                stream
            )
        },
        "5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_string(),
    ];

    for object in &objects {
        offsets.push(pdf.len());
        pdf.push_str(object);
    }

    let xref_start = pdf.len();
    pdf.push_str("xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str("trailer\n<< /Root 1 0 R /Size 6 >>\n");
    pdf.push_str(&format!("startxref\n{xref_start}\n%%EOF"));
    pdf.into_bytes()
}

fn build_minimal_docx_with_text(text: &str) -> Vec<u8> {
    use std::io::Write;

    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("[Content_Types].xml", options).unwrap();
    writer
        .write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
        )
        .unwrap();
    writer.start_file("_rels/.rels", options).unwrap();
    writer
        .write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
        )
        .unwrap();
    writer.start_file("word/document.xml", options).unwrap();
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    writer
        .write_all(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>{escaped}</w:t></w:r></w:p>
  </w:body>
</w:document>"#
            )
            .as_bytes(),
        )
        .unwrap();
    writer.finish().unwrap().into_inner()
}

fn build_minimal_xlsx_with_values() -> Vec<u8> {
    use std::io::Write;

    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("[Content_Types].xml", options).unwrap();
    writer.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
</Types>"#).unwrap();
    writer.start_file("_rels/.rels", options).unwrap();
    writer.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#).unwrap();
    writer.start_file("xl/workbook.xml", options).unwrap();
    writer.write_all(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="预算表" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#.as_bytes()).unwrap();
    writer
        .start_file("xl/_rels/workbook.xml.rels", options)
        .unwrap();
    writer.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#).unwrap();
    writer.start_file("xl/sharedStrings.xml", options).unwrap();
    writer
        .write_all(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2" uniqueCount="2">
  <si><t>项目</t></si>
  <si><t>预算</t></si>
</sst>"#
                .as_bytes(),
        )
        .unwrap();
    writer
        .start_file("xl/worksheets/sheet1.xml", options)
        .unwrap();
    writer
        .write_all(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="s"><v>0</v></c>
      <c r="B1" t="s"><v>1</v></c>
    </row>
    <row r="2">
      <c r="A2" t="str"><v>差旅</v></c>
      <c r="B2"><v>1200</v></c>
    </row>
  </sheetData>
</worksheet>"#
                .as_bytes(),
        )
        .unwrap();
    writer.finish().unwrap().into_inner()
}

fn build_legacy_office_like_bytes(lines: &[&str]) -> Vec<u8> {
    let mut bytes = vec![0xD0, 0xCF, 0x11, 0xE0];
    for line in lines {
        bytes.extend(line.encode_utf16().flat_map(|unit| unit.to_le_bytes()));
        bytes.extend([0x00, 0x00]);
    }
    bytes
}

fn build_minimal_video_with_audio() -> Option<Vec<u8>> {
    let ffmpeg = resolve_ffmpeg_command()?;

    let temp = tempfile::tempdir().ok()?;
    let output_path = temp.path().join("sample.mp4");
    let output = Command::new(&ffmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=160x120:d=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=1000:duration=1",
            "-shortest",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    std::fs::read(output_path).ok()
}

fn build_minimal_video_without_audio() -> Option<Vec<u8>> {
    let ffmpeg = resolve_ffmpeg_command()?;

    let temp = tempfile::tempdir().ok()?;
    let output_path = temp.path().join("sample-no-audio.mp4");
    let output = Command::new(&ffmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=160x120:d=1",
            "-c:v",
            "libx264",
            "-an",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    std::fs::read(output_path).ok()
}

fn build_attachment_input(
    kind: &str,
    source_type: &str,
    name: &str,
    declared_mime_type: Option<&str>,
    size_bytes: Option<usize>,
    source_payload: Option<&str>,
) -> runtime_lib::commands::chat::AttachmentInput {
    runtime_lib::commands::chat::AttachmentInput {
        id: format!("att-{kind}-{}", name.replace('.', "-")),
        kind: kind.to_string(),
        source_type: source_type.to_string(),
        name: name.to_string(),
        declared_mime_type: declared_mime_type.map(str::to_string),
        size_bytes,
        source_payload: source_payload.map(str::to_string),
        source_uri: None,
        extracted_text: None,
        truncated: None,
    }
}

#[test]
fn send_message_request_deserializes_attachment_part_payloads() {
    let request: SendMessageRequest = serde_json::from_value(json!({
        "sessionId": "session-attachment-1",
        "parts": [
            { "type": "text", "text": "请处理附件" },
            {
                "type": "attachment",
                "attachment": {
                    "id": "att-1",
                    "kind": "document",
                    "sourceType": "browser_file",
                    "name": "brief.pdf",
                    "declaredMimeType": "application/pdf",
                    "sizeBytes": 128,
                    "sourcePayload": "data:application/pdf;base64,SGVsbG8="
                }
            }
        ]
    }))
    .expect("request should deserialize");

    assert_eq!(request.session_id, "session-attachment-1");
    assert_eq!(request.parts.len(), 2);

    match &request.parts[1] {
        SendMessagePart::Attachment { attachment } => {
            assert_eq!(attachment.id, "att-1");
            assert_eq!(attachment.kind, "document");
            assert_eq!(attachment.source_type, "browser_file");
            assert_eq!(attachment.name, "brief.pdf");
            assert_eq!(
                attachment.declared_mime_type.as_deref(),
                Some("application/pdf")
            );
            assert_eq!(attachment.size_bytes, Some(128));
            assert_eq!(
                attachment.source_payload.as_deref(),
                Some("data:application/pdf;base64,SGVsbG8=")
            );
        }
        other => panic!("expected attachment part, got {other:?}"),
    }
}

#[test]
fn send_message_request_still_deserializes_legacy_file_text_parts() {
    let request: SendMessageRequest = serde_json::from_value(json!({
        "sessionId": "session-legacy-1",
        "parts": [
            { "type": "text", "text": "请处理旧格式附件" },
            {
                "type": "file_text",
                "name": "notes.md",
                "mimeType": "text/markdown",
                "size": 64,
                "text": "# brief",
                "truncated": false
            }
        ]
    }))
    .expect("legacy request should deserialize");

    assert_eq!(request.session_id, "session-legacy-1");

    match &request.parts[1] {
        SendMessagePart::FileText {
            name,
            mime_type,
            size,
            text,
            truncated,
        } => {
            assert_eq!(name, "notes.md");
            assert_eq!(mime_type, "text/markdown");
            assert_eq!(*size, 64);
            assert_eq!(text, "# brief");
            assert_eq!(*truncated, Some(false));
        }
        other => panic!("expected legacy file_text part, got {other:?}"),
    }
}

#[test]
fn attachment_parts_normalize_image_inputs_to_legacy_image_parts() {
    let parts =
        runtime_lib::commands::chat::normalize_send_message_parts(&[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-image-1".to_string(),
                kind: "image".to_string(),
                source_type: "browser_file".to_string(),
                name: "screen.png".to_string(),
                declared_mime_type: Some("image/png".to_string()),
                size_bytes: Some(12),
                source_payload: Some("data:image/png;base64,aGVsbG8=".to_string()),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }])
        .expect("normalize image attachment");

    assert_eq!(parts[0]["type"].as_str(), Some("image"));
    assert_eq!(parts[0]["name"].as_str(), Some("screen.png"));
    assert_eq!(parts[0]["mimeType"].as_str(), Some("image/png"));
}

#[test]
fn attachment_parts_normalize_text_documents_to_legacy_file_text_parts() {
    let parts =
        runtime_lib::commands::chat::normalize_send_message_parts(&[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-doc-1".to_string(),
                kind: "document".to_string(),
                source_type: "browser_file".to_string(),
                name: "notes.md".to_string(),
                declared_mime_type: Some("text/markdown".to_string()),
                size_bytes: Some(64),
                source_payload: Some("# brief".to_string()),
                source_uri: None,
                extracted_text: None,
                truncated: Some(false),
            },
        }])
        .expect("normalize text attachment");

    assert_eq!(parts[0]["type"].as_str(), Some("file_text"));
    assert_eq!(parts[0]["name"].as_str(), Some("notes.md"));
    assert_eq!(parts[0]["text"].as_str(), Some("# brief"));
}

#[test]
fn attachment_parts_truncate_large_text_documents() {
    let oversized_text = "A".repeat(200_001);
    let parts =
        runtime_lib::commands::chat::normalize_send_message_parts(&[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-doc-large-1".to_string(),
                kind: "document".to_string(),
                source_type: "browser_file".to_string(),
                name: "huge.md".to_string(),
                declared_mime_type: Some("text/markdown".to_string()),
                size_bytes: Some(oversized_text.len()),
                source_payload: Some(oversized_text),
                source_uri: None,
                extracted_text: None,
                truncated: Some(false),
            },
        }])
        .expect("normalize oversized text attachment");

    assert_eq!(parts[0]["type"].as_str(), Some("file_text"));
    let text = parts[0]["text"].as_str().expect("text");
    assert_eq!(text.len(), 200_000);
    assert_eq!(parts[0]["truncated"].as_bool(), Some(true));
}

#[test]
fn attachment_parts_normalize_pdf_documents_to_legacy_pdf_parts() {
    let pdf_data =
        base64::engine::general_purpose::STANDARD.encode(build_minimal_pdf_with_text("Hello PDF"));
    let parts =
        runtime_lib::commands::chat::normalize_send_message_parts(&[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-pdf-1".to_string(),
                kind: "document".to_string(),
                source_type: "browser_file".to_string(),
                name: "brief.pdf".to_string(),
                declared_mime_type: Some("application/pdf".to_string()),
                size_bytes: Some(128),
                source_payload: Some(pdf_data),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }])
        .expect("normalize pdf attachment");

    assert_eq!(parts[0]["type"].as_str(), Some("pdf_file"));
    assert_eq!(parts[0]["name"].as_str(), Some("brief.pdf"));
    assert!(
        parts[0]["extractedText"]
            .as_str()
            .expect("extracted text")
            .contains("Hello PDF")
    );
}

#[test]
fn attachment_parts_preserve_audio_inputs_as_unified_attachment_parts() {
    let parts =
        runtime_lib::commands::chat::normalize_send_message_parts(&[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-audio-1".to_string(),
                kind: "audio".to_string(),
                source_type: "browser_file".to_string(),
                name: "call.mp3".to_string(),
                declared_mime_type: Some("audio/mpeg".to_string()),
                size_bytes: Some(256),
                source_payload: Some("ZmFrZQ==".to_string()),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }])
        .expect("normalize audio attachment");

    assert_eq!(parts[0]["type"].as_str(), Some("attachment"));
    assert_eq!(parts[0]["attachment"]["kind"].as_str(), Some("audio"));
    assert_eq!(
        parts[0]["attachment"]["transcript"].as_str(),
        Some("TRANSCRIPTION_REQUIRED")
    );
}

#[test]
fn attachment_parts_preserve_binary_document_inputs_as_unified_attachment_parts() {
    let parts =
        runtime_lib::commands::chat::normalize_send_message_parts(&[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-sheet-1".to_string(),
                kind: "document".to_string(),
                source_type: "browser_file".to_string(),
                name: "budget.xlsx".to_string(),
                declared_mime_type: Some(
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
                ),
                size_bytes: Some(256),
                source_payload: None,
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }])
        .expect("normalize binary document attachment");

    assert_eq!(parts[0]["type"].as_str(), Some("attachment"));
    assert_eq!(parts[0]["attachment"]["kind"].as_str(), Some("document"));
    assert_eq!(
        parts[0]["attachment"]["summary"].as_str(),
        Some("EXTRACTION_REQUIRED")
    );
    assert!(
        parts[0]["attachment"]["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning.as_str() == Some("document_extraction_pending"))
    );
}

#[test]
fn attachment_parts_extract_docx_documents_to_text_parts() {
    let payload = base64::engine::general_purpose::STANDARD
        .encode(build_minimal_docx_with_text("WorkClaw 文档内容"));
    let parts =
        runtime_lib::commands::chat::normalize_send_message_parts(&[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-docx-1".to_string(),
                kind: "document".to_string(),
                source_type: "browser_file".to_string(),
                name: "brief.docx".to_string(),
                declared_mime_type: Some(
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                        .to_string(),
                ),
                size_bytes: Some(256),
                source_payload: Some(payload),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }])
        .expect("normalize docx attachment");

    assert_eq!(parts[0]["type"].as_str(), Some("file_text"));
    assert!(
        parts[0]["text"]
            .as_str()
            .expect("text")
            .contains("WorkClaw 文档内容")
    );
}

#[test]
fn attachment_parts_extract_xlsx_documents_to_text_parts() {
    let payload =
        base64::engine::general_purpose::STANDARD.encode(build_minimal_xlsx_with_values());
    let parts =
        runtime_lib::commands::chat::normalize_send_message_parts(&[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-xlsx-1".to_string(),
                kind: "document".to_string(),
                source_type: "browser_file".to_string(),
                name: "budget.xlsx".to_string(),
                declared_mime_type: Some(
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
                ),
                size_bytes: Some(256),
                source_payload: Some(payload),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }])
        .expect("normalize xlsx attachment");

    assert_eq!(parts[0]["type"].as_str(), Some("file_text"));
    let text = parts[0]["text"].as_str().expect("text");
    assert!(text.contains("工作表: 预算表"));
    assert!(text.contains("A2: 差旅"));
    assert!(text.contains("B2: 1200"));
}

#[test]
fn attachment_parts_extract_legacy_doc_documents_to_text_parts() {
    let payload =
        base64::engine::general_purpose::STANDARD.encode(build_legacy_office_like_bytes(&[
            "旧版文档标题",
            "第一段正文",
        ]));
    let parts =
        runtime_lib::commands::chat::normalize_send_message_parts(&[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-doc-legacy-1".to_string(),
                kind: "document".to_string(),
                source_type: "browser_file".to_string(),
                name: "legacy.doc".to_string(),
                declared_mime_type: Some("application/msword".to_string()),
                size_bytes: Some(256),
                source_payload: Some(payload),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }])
        .expect("normalize legacy doc attachment");

    assert_eq!(parts[0]["type"].as_str(), Some("file_text"));
    let text = parts[0]["text"].as_str().expect("text");
    assert!(text.contains("旧版文档标题"));
    assert!(text.contains("第一段正文"));
}

#[test]
fn attachment_parts_extract_legacy_xls_documents_to_text_parts() {
    let payload = base64::engine::general_purpose::STANDARD
        .encode(build_legacy_office_like_bytes(&["预算汇总", "差旅 1200"]));
    let parts =
        runtime_lib::commands::chat::normalize_send_message_parts(&[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-xls-legacy-1".to_string(),
                kind: "document".to_string(),
                source_type: "browser_file".to_string(),
                name: "legacy.xls".to_string(),
                declared_mime_type: Some("application/vnd.ms-excel".to_string()),
                size_bytes: Some(256),
                source_payload: Some(payload),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }])
        .expect("normalize legacy xls attachment");

    assert_eq!(parts[0]["type"].as_str(), Some("file_text"));
    let text = parts[0]["text"].as_str().expect("text");
    assert!(text.contains("预算汇总"));
    assert!(text.contains("差旅 1200"));
}

#[test]
fn validation_rejects_unsupported_source_type_for_attachment() {
    let attachment = build_attachment_input(
        "image",
        "local_path",
        "screen.png",
        Some("image/png"),
        Some(128),
        Some("data:image/png;base64,aGVsbG8="),
    );

    let error = validate_attachment_input(&default_attachment_policy(), &attachment)
        .expect_err("unsupported source types should fail");

    assert!(error.contains("sourceType"));
    assert!(error.contains("local_path"));
}

#[test]
fn validation_accepts_audio_attachments_for_fallback_in_current_phase() {
    let attachment = build_attachment_input(
        "audio",
        "browser_file",
        "memo.mp3",
        Some("audio/mpeg"),
        Some(256),
        Some("ZmFrZQ=="),
    );

    validate_attachment_input(&default_attachment_policy(), &attachment)
        .expect("audio should be accepted for fallback");
}

#[test]
fn validation_accepts_binary_documents_for_fallback_in_current_phase() {
    let attachment = build_attachment_input(
        "document",
        "browser_file",
        "budget.xlsx",
        Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        Some(256),
        None,
    );

    validate_attachment_input(&default_attachment_policy(), &attachment)
        .expect("binary documents should be accepted for fallback");
}

#[test]
fn validation_rejects_oversized_image_attachment() {
    let attachment = build_attachment_input(
        "image",
        "browser_file",
        "huge.png",
        Some("image/png"),
        Some((5 * 1024 * 1024) + 1),
        Some("data:image/png;base64,aGVsbG8="),
    );

    let error = validate_attachment_input(&default_attachment_policy(), &attachment)
        .expect_err("oversized image should fail");

    assert!(error.contains("huge.png"));
    assert!(error.contains("5242880"));
}

#[test]
fn validation_rejects_oversized_image_attachment_without_declared_size() {
    let oversized_payload = "A".repeat((5 * 1024 * 1024) + 1);
    let encoded = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(oversized_payload)
    );
    let attachment = build_attachment_input(
        "image",
        "browser_file",
        "implicit-size.png",
        Some("image/png"),
        None,
        Some(&encoded),
    );

    let error = validate_attachment_input(&default_attachment_policy(), &attachment)
        .expect_err("oversized derived image should fail");

    assert!(error.contains("implicit-size.png"));
    assert!(error.contains("5242880"));
}

#[test]
fn validation_rejects_image_attachments_that_exceed_total_payload_budget() {
    let payload = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(vec![0_u8; 4 * 1024 * 1024])
    );
    let parts = (1..=3)
        .map(|index| SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: format!("att-image-total-{index}"),
                kind: "image".to_string(),
                source_type: "browser_file".to_string(),
                name: format!("large-{index}.png"),
                declared_mime_type: Some("image/png".to_string()),
                size_bytes: Some(4 * 1024 * 1024),
                source_payload: Some(payload.clone()),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        })
        .collect::<Vec<_>>();

    let error = runtime_lib::commands::chat::normalize_send_message_parts(&parts)
        .expect_err("image batch should respect the total payload budget");

    assert!(error.contains("图片附件总大小"));
    assert!(error.contains("10485760"));
}

#[test]
fn validation_rejects_oversized_document_attachment() {
    let attachment = build_attachment_input(
        "document",
        "browser_file",
        "notes.md",
        Some("text/markdown"),
        Some((20 * 1024 * 1024) + 1),
        Some("# brief"),
    );

    let error = validate_attachment_input(&default_attachment_policy(), &attachment)
        .expect_err("oversized text document should fail");

    assert!(error.contains("notes.md"));
    assert!(error.contains("20971520"));
}

#[test]
fn mixed_legacy_and_unified_attachments_still_respect_global_limits() {
    let error = runtime_lib::commands::chat::normalize_send_message_parts(&[
        SendMessagePart::Image {
            name: "legacy-1.png".to_string(),
            mime_type: "image/png".to_string(),
            size: 12,
            data: "data:image/png;base64,aGVsbG8=".to_string(),
        },
        SendMessagePart::Image {
            name: "legacy-2.png".to_string(),
            mime_type: "image/png".to_string(),
            size: 12,
            data: "data:image/png;base64,aGVsbG8=".to_string(),
        },
        SendMessagePart::Image {
            name: "legacy-3.png".to_string(),
            mime_type: "image/png".to_string(),
            size: 12,
            data: "data:image/png;base64,aGVsbG8=".to_string(),
        },
        SendMessagePart::FileText {
            name: "legacy-4.md".to_string(),
            mime_type: "text/markdown".to_string(),
            size: 8,
            text: "# four".to_string(),
            truncated: Some(false),
        },
        SendMessagePart::FileText {
            name: "legacy-5.md".to_string(),
            mime_type: "text/markdown".to_string(),
            size: 8,
            text: "# five".to_string(),
            truncated: Some(false),
        },
        SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-6".to_string(),
                kind: "document".to_string(),
                source_type: "browser_file".to_string(),
                name: "new-six.md".to_string(),
                declared_mime_type: Some("text/markdown".to_string()),
                size_bytes: Some(8),
                source_payload: Some("# six".to_string()),
                source_uri: None,
                extracted_text: None,
                truncated: Some(false),
            },
        },
    ])
    .expect_err("mixed payload should still respect the global attachment cap");

    assert!(error.contains("附件数量超过当前阶段限制 5"));
}

#[test]
fn attachment_parts_truncate_provided_pdf_extracted_text() {
    let extracted_text = "A".repeat(200_001);
    let parts =
        runtime_lib::commands::chat::normalize_send_message_parts(&[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-pdf-inline-1".to_string(),
                kind: "document".to_string(),
                source_type: "browser_file".to_string(),
                name: "brief.pdf".to_string(),
                declared_mime_type: Some("application/pdf".to_string()),
                size_bytes: None,
                source_payload: None,
                source_uri: None,
                extracted_text: Some(extracted_text),
                truncated: Some(false),
            },
        }])
        .expect("normalize pdf attachment with inline extracted text");

    let normalized_text = parts[0]["extractedText"].as_str().expect("extracted text");
    assert_eq!(normalized_text.len(), 200_000);
    assert_eq!(parts[0]["truncated"].as_bool(), Some(true));
}

#[test]
fn resolution_preserves_attachment_kind_and_mime_metadata_for_supported_inputs() {
    let image = resolve_attachment_input(
        &default_attachment_policy(),
        &build_attachment_input(
            "image",
            "browser_file",
            "screen.png",
            Some("image/png"),
            Some(128),
            Some("data:image/png;base64,aGVsbG8="),
        ),
    )
    .expect("resolve image");

    assert_eq!(image.kind, "image");
    assert_eq!(image.resolved_mime_type, "image/png");
    assert_eq!(image.size_bytes, Some(128));

    let document = resolve_attachment_input(
        &default_attachment_policy(),
        &build_attachment_input(
            "document",
            "browser_file",
            "notes.md",
            Some("text/markdown"),
            Some(64),
            Some("# brief"),
        ),
    )
    .expect("resolve document");

    assert_eq!(document.kind, "document");
    assert_eq!(document.resolved_mime_type, "text/markdown");
    assert_eq!(document.size_bytes, Some(64));

    let audio = resolve_attachment_input(
        &default_attachment_policy(),
        &build_attachment_input(
            "audio",
            "browser_file",
            "memo.mp3",
            Some("audio/mpeg"),
            Some(128),
            Some("ZmFrZQ=="),
        ),
    )
    .expect("resolve audio");

    assert_eq!(audio.kind, "audio");
    assert_eq!(audio.resolved_mime_type, "audio/mpeg");
    assert_eq!(audio.transcript.as_deref(), Some("TRANSCRIPTION_REQUIRED"));
    assert!(
        audio
            .warnings
            .iter()
            .any(|warning| warning == "transcription_pending")
    );

    let video = resolve_attachment_input(
        &default_attachment_policy(),
        &build_attachment_input(
            "video",
            "browser_file",
            "demo.mp4",
            Some("video/mp4"),
            Some(256),
            Some("ZmFrZV92aWRlbw=="),
        ),
    )
    .expect("resolve video");

    assert_eq!(video.kind, "video");
    assert_eq!(video.resolved_mime_type, "video/mp4");
    assert_eq!(video.summary.as_deref(), Some("SUMMARY_REQUIRED"));
    assert!(
        video
            .warnings
            .iter()
            .any(|warning| warning == "summary_pending")
    );

    let no_audio_video = resolve_attachment_input(
        &default_attachment_policy(),
        &runtime_lib::commands::chat::AttachmentInput {
            extracted_text: Some("VIDEO_NO_AUDIO_TRACK".to_string()),
            ..build_attachment_input(
                "video",
                "browser_file",
                "silent.mp4",
                Some("video/mp4"),
                Some(256),
                Some("ZmFrZV92aWRlbw=="),
            )
        },
    )
    .expect("resolve no-audio video");

    assert_eq!(
        no_audio_video.summary.as_deref(),
        Some("VIDEO_NO_AUDIO_TRACK")
    );
    assert!(
        no_audio_video
            .warnings
            .iter()
            .any(|warning| warning == "video_no_audio_track")
    );

    let binary_document = resolve_attachment_input(
        &default_attachment_policy(),
        &build_attachment_input(
            "document",
            "browser_file",
            "budget.xlsx",
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
            Some(256),
            None,
        ),
    )
    .expect("resolve binary document");

    assert_eq!(binary_document.kind, "document");
    assert_eq!(
        binary_document.summary.as_deref(),
        Some("EXTRACTION_REQUIRED")
    );
    assert!(
        binary_document
            .warnings
            .iter()
            .any(|warning| warning == "document_extraction_pending")
    );
}

#[tokio::test]
async fn async_normalize_send_message_parts_transcribes_audio_when_audio_route_exists() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO routing_policies (capability, primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, enabled)
         VALUES ('audio_stt', 'provider-audio', 'gpt-4o-mini-transcribe', '[]', 60000, 0, 1)",
    )
    .execute(&pool)
    .await
    .expect("insert audio route");
    sqlx::query(
        "INSERT INTO provider_configs (id, provider_key, display_name, protocol_type, base_url, auth_type, api_key_encrypted, org_id, extra_json, enabled, created_at, updated_at)
         VALUES ('provider-audio', 'openai', 'OpenAI Audio', 'openai', 'http://mock-audio-stt-success', 'api_key', 'sk-audio', '', '{}', 1, '2026-04-19T00:00:00Z', '2026-04-19T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .expect("insert audio provider");

    let parts = normalize_send_message_parts_with_pool(
        &[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-audio-async-1".to_string(),
                kind: "audio".to_string(),
                source_type: "browser_file".to_string(),
                name: "memo.mp3".to_string(),
                declared_mime_type: Some("audio/mpeg".to_string()),
                size_bytes: Some(128),
                source_payload: Some("ZmFrZQ==".to_string()),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }],
        &pool,
    )
    .await
    .expect("normalize audio via async path");

    assert_eq!(parts[0]["type"].as_str(), Some("attachment"));
    assert_eq!(
        parts[0]["attachment"]["transcript"].as_str(),
        Some("MOCK_TRANSCRIPT: memo.mp3")
    );
    assert_eq!(
        parts[0]["attachment"]["warnings"]
            .as_array()
            .expect("warnings")
            .len(),
        0
    );
}

#[tokio::test]
async fn async_normalize_send_message_parts_transcribes_audio_with_qwen_openai_compatible_route() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO routing_policies (capability, primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, enabled)
         VALUES ('audio_stt', 'provider-audio', 'paraformer-v2', '[]', 60000, 0, 1)",
    )
    .execute(&pool)
    .await
    .expect("insert audio route");
    sqlx::query(
        "INSERT INTO provider_configs (id, provider_key, display_name, protocol_type, base_url, auth_type, api_key_encrypted, org_id, extra_json, enabled, created_at, updated_at)
         VALUES ('provider-audio', 'qwen', 'Qwen Audio', 'openai', 'http://mock-audio-stt-success', 'api_key', 'sk-audio', '', '{}', 1, '2026-04-19T00:00:00Z', '2026-04-19T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .expect("insert qwen audio provider");

    let parts = normalize_send_message_parts_with_pool(
        &[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-audio-qwen-async-1".to_string(),
                kind: "audio".to_string(),
                source_type: "browser_file".to_string(),
                name: "meeting.wav".to_string(),
                declared_mime_type: Some("audio/wav".to_string()),
                size_bytes: Some(128),
                source_payload: Some("ZmFrZQ==".to_string()),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }],
        &pool,
    )
    .await
    .expect("normalize qwen audio via async path");

    assert_eq!(
        parts[0]["attachment"]["transcript"].as_str(),
        Some("MOCK_TRANSCRIPT: meeting.wav")
    );
    assert_eq!(
        parts[0]["attachment"]["warnings"]
            .as_array()
            .expect("warnings")
            .len(),
        0
    );
}

#[tokio::test]
async fn async_normalize_send_message_parts_keeps_pending_audio_without_audio_route() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let parts = normalize_send_message_parts_with_pool(
        &[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-audio-async-2".to_string(),
                kind: "audio".to_string(),
                source_type: "browser_file".to_string(),
                name: "call.mp3".to_string(),
                declared_mime_type: Some("audio/mpeg".to_string()),
                size_bytes: Some(128),
                source_payload: Some("ZmFrZQ==".to_string()),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }],
        &pool,
    )
    .await
    .expect("normalize pending audio via async path");

    assert_eq!(
        parts[0]["attachment"]["transcript"].as_str(),
        Some("TRANSCRIPTION_REQUIRED")
    );
    assert!(
        parts[0]["attachment"]["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning.as_str() == Some("transcription_pending"))
    );
}

#[tokio::test]
async fn async_normalize_send_message_parts_summarizes_video_when_audio_route_and_ffmpeg_exist() {
    let Some(video_bytes) = build_minimal_video_with_audio() else {
        return;
    };

    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO routing_policies (capability, primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, enabled)
         VALUES ('audio_stt', 'provider-audio', 'gpt-4o-mini-transcribe', '[]', 60000, 0, 1)",
    )
    .execute(&pool)
    .await
    .expect("insert audio route");
    sqlx::query(
        "INSERT INTO provider_configs (id, provider_key, display_name, protocol_type, base_url, auth_type, api_key_encrypted, org_id, extra_json, enabled, created_at, updated_at)
         VALUES ('provider-audio', 'openai', 'OpenAI Audio', 'openai', 'http://mock-audio-stt-success', 'api_key', 'sk-audio', '', '{}', 1, '2026-04-19T00:00:00Z', '2026-04-19T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .expect("insert audio provider");

    let parts = normalize_send_message_parts_with_pool(
        &[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-video-async-1".to_string(),
                kind: "video".to_string(),
                source_type: "browser_file".to_string(),
                name: "demo.mp4".to_string(),
                declared_mime_type: Some("video/mp4".to_string()),
                size_bytes: Some(video_bytes.len()),
                source_payload: Some(base64::engine::general_purpose::STANDARD.encode(video_bytes)),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }],
        &pool,
    )
    .await
    .expect("normalize video via async path");

    assert_eq!(parts[0]["type"].as_str(), Some("attachment"));
    let summary = parts[0]["attachment"]["summary"].as_str().expect("summary");
    assert!(summary.contains("音轨转写"));
    assert!(summary.contains("demo.mp4"));
    assert_eq!(
        parts[0]["attachment"]["warnings"]
            .as_array()
            .expect("warnings")
            .len(),
        0
    );
}

#[tokio::test]
async fn async_normalize_send_message_parts_marks_video_without_audio_track_explicitly() {
    let Some(video_bytes) = build_minimal_video_without_audio() else {
        return;
    };

    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO routing_policies (capability, primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, enabled)
         VALUES ('audio_stt', 'provider-audio', 'gpt-4o-mini-transcribe', '[]', 60000, 0, 1)",
    )
    .execute(&pool)
    .await
    .expect("insert audio route");
    sqlx::query(
        "INSERT INTO provider_configs (id, provider_key, display_name, protocol_type, base_url, auth_type, api_key_encrypted, org_id, extra_json, enabled, created_at, updated_at)
         VALUES ('provider-audio', 'openai', 'OpenAI Audio', 'openai', 'http://mock-audio-stt-success', 'api_key', 'sk-audio', '', '{}', 1, '2026-04-19T00:00:00Z', '2026-04-19T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .expect("insert audio provider");

    let parts = normalize_send_message_parts_with_pool(
        &[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-video-async-no-audio-1".to_string(),
                kind: "video".to_string(),
                source_type: "browser_file".to_string(),
                name: "silent.mp4".to_string(),
                declared_mime_type: Some("video/mp4".to_string()),
                size_bytes: Some(video_bytes.len()),
                source_payload: Some(base64::engine::general_purpose::STANDARD.encode(video_bytes)),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }],
        &pool,
    )
    .await
    .expect("normalize silent video via async path");

    assert_eq!(parts[0]["type"].as_str(), Some("attachment"));
    assert_eq!(
        parts[0]["attachment"]["summary"].as_str(),
        Some("VIDEO_NO_AUDIO_TRACK")
    );
    assert!(
        parts[0]["attachment"]["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning.as_str() == Some("video_no_audio_track"))
    );
}

#[tokio::test]
async fn async_normalize_send_message_parts_summarizes_silent_video_with_vision_route() {
    let Some(video_bytes) = build_minimal_video_without_audio() else {
        return;
    };

    let (pool, _tmp) = helpers::setup_test_db().await;
    sqlx::query(
        "INSERT INTO routing_policies (capability, primary_provider_id, primary_model, fallback_chain_json, timeout_ms, retry_count, enabled)
         VALUES ('vision', 'provider-vision', 'qwen-vl-max', '[]', 60000, 0, 1)",
    )
    .execute(&pool)
    .await
    .expect("insert vision route");
    sqlx::query(
        "INSERT INTO provider_configs (id, provider_key, display_name, protocol_type, base_url, auth_type, api_key_encrypted, org_id, extra_json, enabled, created_at, updated_at)
         VALUES ('provider-vision', 'qwen', 'Qwen Vision', 'openai', 'http://mock-vision-summary-success', 'api_key', 'sk-vision', '', '{}', 1, '2026-04-19T00:00:00Z', '2026-04-19T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .expect("insert vision provider");

    let parts = normalize_send_message_parts_with_pool(
        &[SendMessagePart::Attachment {
            attachment: runtime_lib::commands::chat::AttachmentInput {
                id: "att-video-vision-1".to_string(),
                kind: "video".to_string(),
                source_type: "browser_file".to_string(),
                name: "silent-vision.mp4".to_string(),
                declared_mime_type: Some("video/mp4".to_string()),
                size_bytes: Some(video_bytes.len()),
                source_payload: Some(base64::engine::general_purpose::STANDARD.encode(video_bytes)),
                source_uri: None,
                extracted_text: None,
                truncated: None,
            },
        }],
        &pool,
    )
    .await
    .expect("normalize silent video with vision route");

    let summary = parts[0]["attachment"]["summary"].as_str().expect("summary");
    assert!(summary.contains("视频画面摘要"));
    assert!(summary.contains("MOCK_VISION_SUMMARY"));
    assert_eq!(
        parts[0]["attachment"]["warnings"]
            .as_array()
            .expect("warnings")
            .len(),
        0
    );
}
