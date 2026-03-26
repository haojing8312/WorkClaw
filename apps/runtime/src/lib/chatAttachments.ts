import type { PendingAttachment } from "../types";

export const MAX_FILES = 5;
export const MAX_IMAGE_FILES = 3;
export const MAX_IMAGE_SIZE = 5 * 1024 * 1024;
export const MAX_TEXT_FILE_SIZE = 1 * 1024 * 1024;
export const MAX_PDF_FILE_SIZE = 10 * 1024 * 1024;

const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "webp"]);
const TEXT_FILE_EXTENSIONS = new Set([
  "txt",
  "md",
  "json",
  "yaml",
  "yml",
  "xml",
  "csv",
  "tsv",
  "log",
  "ini",
  "conf",
  "env",
  "js",
  "jsx",
  "ts",
  "tsx",
  "py",
  "rs",
  "go",
  "java",
  "c",
  "cpp",
  "h",
  "cs",
  "sh",
  "ps1",
  "sql",
]);
const PDF_EXTENSIONS = new Set(["pdf"]);

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
  if (file.kind === "pdf-file") {
    return { badge: "PDF", truncated: Boolean(file.truncated) };
  }
  return { badge: "文本", truncated: Boolean(file.truncated) };
}
