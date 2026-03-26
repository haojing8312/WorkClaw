use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};

use super::chat::SendMessagePart;

const MAX_PDF_EXTRACTED_TEXT_CHARS: usize = 200_000;

pub(crate) fn normalize_message_parts(parts: &[SendMessagePart]) -> Result<Vec<Value>, String> {
    parts.iter().map(normalize_message_part).collect()
}

fn normalize_message_part(part: &SendMessagePart) -> Result<Value, String> {
    match part {
        SendMessagePart::Text { text } => Ok(json!({
            "type": "text",
            "text": text,
        })),
        SendMessagePart::Image {
            name,
            mime_type,
            size,
            data,
        } => Ok(json!({
            "type": "image",
            "name": name,
            "mimeType": mime_type,
            "size": size,
            "data": data,
        })),
        SendMessagePart::FileText {
            name,
            mime_type,
            size,
            text,
            truncated,
        } => Ok(json!({
            "type": "file_text",
            "name": name,
            "mimeType": mime_type,
            "size": size,
            "text": text,
            "truncated": truncated.unwrap_or(false),
        })),
        SendMessagePart::PdfFile {
            name,
            mime_type,
            size,
            data,
        } => {
            let (extracted_text, truncated) = extract_pdf_text(data).map_err(|err| {
                format!("PDF 文件 {name} 解析失败: {err}")
            })?;
            Ok(json!({
                "type": "pdf_file",
                "name": name,
                "mimeType": mime_type,
                "size": size,
                "extractedText": extracted_text,
                "truncated": truncated,
            }))
        }
    }
}

fn extract_pdf_text(data: &str) -> Result<(String, bool), String> {
    let payload = data
        .split_once("base64,")
        .map(|(_, payload)| payload)
        .unwrap_or(data);
    let bytes = BASE64.decode(payload).map_err(|err| err.to_string())?;
    let extracted = pdf_extract::extract_text_from_mem(&bytes).map_err(|err| err.to_string())?;
    let normalized = extracted
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if normalized.is_empty() {
        return Ok(("未提取到可读的 PDF 文本内容。".to_string(), false));
    }

    let mut iter = normalized.chars();
    let excerpt: String = iter.by_ref().take(MAX_PDF_EXTRACTED_TEXT_CHARS).collect();
    let truncated = iter.next().is_some();
    Ok((excerpt, truncated))
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use super::normalize_message_parts;
    use crate::commands::chat::SendMessagePart;

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

    #[test]
    fn normalize_message_parts_extracts_pdf_payload_to_text() {
        let pdf_data = BASE64.encode(build_minimal_pdf_with_text("Hello PDF"));
        let parts = normalize_message_parts(&[SendMessagePart::PdfFile {
            name: "brief.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            size: 652,
            data: pdf_data,
        }])
        .expect("normalize");

        assert_eq!(parts[0]["type"].as_str(), Some("pdf_file"));
        assert_eq!(parts[0]["name"].as_str(), Some("brief.pdf"));
        assert!(
            parts[0]["extractedText"]
                .as_str()
                .expect("extracted text")
                .contains("Hello PDF")
        );
    }
}
