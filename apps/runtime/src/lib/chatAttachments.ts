import type { PendingAttachment } from "../types";
export {
  MAX_FILES,
  MAX_IMAGE_FILES,
  MAX_IMAGE_SIZE,
  MAX_PDF_FILE_SIZE,
  MAX_TEXT_FILE_SIZE,
  MAX_TEXT_PREVIEW_CHARS,
} from "./attachmentPolicy";
import {
  BINARY_DOCUMENT_EXTENSIONS as BINARY_DOCUMENT_EXTENSIONS_LIST,
  IMAGE_EXTENSIONS as IMAGE_EXTENSIONS_LIST,
  PDF_EXTENSIONS as PDF_EXTENSIONS_LIST,
  TEXT_FILE_EXTENSIONS as TEXT_FILE_EXTENSIONS_LIST,
} from "./attachmentPolicy";

const IMAGE_EXTENSIONS: ReadonlySet<string> = new Set(IMAGE_EXTENSIONS_LIST);
const TEXT_FILE_EXTENSIONS: ReadonlySet<string> = new Set(TEXT_FILE_EXTENSIONS_LIST);
const PDF_EXTENSIONS: ReadonlySet<string> = new Set(PDF_EXTENSIONS_LIST);
const BINARY_DOCUMENT_EXTENSIONS: ReadonlySet<string> = new Set(BINARY_DOCUMENT_EXTENSIONS_LIST);

export function getFileExtension(fileName: string): string {
  return fileName.split(".").pop()?.toLowerCase() ?? "";
}

export function isImageFile(file: File): boolean {
  return file.type.startsWith("image/") || IMAGE_EXTENSIONS.has(getFileExtension(file.name));
}

export function isTextFile(file: File): boolean {
  return TEXT_FILE_EXTENSIONS.has(getFileExtension(file.name));
}

export function isPdfFile(file: File): boolean {
  return file.type === "application/pdf" || PDF_EXTENSIONS.has(getFileExtension(file.name));
}

export function isBinaryDocumentFile(file: File): boolean {
  return BINARY_DOCUMENT_EXTENSIONS.has(getFileExtension(file.name));
}

export function createAttachmentId(prefix = "attachment"): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(typeof reader.result === "string" ? reader.result : "");
    reader.onerror = () => reject(reader.error ?? new Error("文件读取失败"));
    reader.readAsDataURL(file);
  });
}

export function readFileAsText(file: File): Promise<string> {
  if (typeof file.text === "function") {
    return file.text();
  }

  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(typeof reader.result === "string" ? reader.result : "");
    reader.onerror = () => reject(reader.error ?? new Error("文件读取失败"));
    reader.readAsText(file);
  });
}

export async function readFileAsBase64(file: File): Promise<string> {
  const dataUrl = await readFileAsDataUrl(file);
  const [, payload = ""] = dataUrl.split("base64,");
  return payload;
}

export function buildPendingAttachmentMeta(file: PendingAttachment): { badge: string; truncated: boolean } {
  if (file.kind === "image") {
    return { badge: "图片", truncated: false };
  }
  if (file.kind === "audio") {
    return { badge: "音频", truncated: false };
  }
  if (file.kind === "video") {
    return { badge: "视频", truncated: false };
  }
  if (file.kind === "pdf-file") {
    return { badge: "PDF", truncated: Boolean(file.truncated) };
  }
  if (file.kind === "document-file") {
    return { badge: "文档", truncated: false };
  }
  return { badge: "文本", truncated: Boolean(file.truncated) };
}
