import { describe, expect, test } from "vitest";
import { normalizeBrowserFileAttachmentDrafts } from "../attachmentDrafts";
import { type AttachmentPolicy } from "../attachmentPolicy";

function buildPolicy(): AttachmentPolicy {
  return {
    maxFiles: 8,
    kinds: {
      image: {
        enabled: true,
        maxCount: 2,
        fileTypes: [
          {
            mimeTypes: ["image/*"],
            extensions: ["png", "jpg"],
            maxSizeBytes: 5 * 1024 * 1024,
          },
        ],
      },
      audio: {
        enabled: true,
        maxCount: 2,
        fileTypes: [
          {
            mimeTypes: ["audio/*"],
            extensions: ["mp3", "wav"],
            maxSizeBytes: 25 * 1024 * 1024,
          },
        ],
      },
      video: {
        enabled: true,
        maxCount: 2,
        fileTypes: [
          {
            mimeTypes: ["video/*"],
            extensions: ["mp4", "mov"],
            maxSizeBytes: 100 * 1024 * 1024,
          },
        ],
      },
      document: {
        enabled: true,
        fileTypes: [
          {
            mimeTypes: ["text/plain"],
            extensions: ["txt", "md"],
            maxSizeBytes: 1 * 1024 * 1024,
          },
          {
            mimeTypes: ["application/pdf"],
            extensions: ["pdf"],
            maxSizeBytes: 10 * 1024 * 1024,
          },
          {
            mimeTypes: ["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"],
            extensions: ["xlsx"],
            maxSizeBytes: 20 * 1024 * 1024,
          },
        ],
      },
    },
  };
}

describe("normalizeBrowserFileAttachmentDrafts", () => {
  test("normalizes image, audio, video, and document browser files into unified drafts", () => {
    const result = normalizeBrowserFileAttachmentDrafts(
      [
        { sourceType: "browser_file", file: { name: "photo.PNG", type: "image/png", size: 1024 } },
        { sourceType: "browser_file", file: { name: "voice.mp3", type: "audio/mpeg", size: 2048 } },
        { sourceType: "browser_file", file: { name: "clip.mov", type: "video/quicktime", size: 4096 } },
        { sourceType: "browser_file", file: { name: "report.pdf", type: "application/pdf", size: 8192 } },
        {
          sourceType: "browser_file",
          file: {
            name: "budget.xlsx",
            type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            size: 4096,
          },
        },
      ],
      buildPolicy(),
    );

    expect(result.rejected).toEqual([]);
    expect(result.accepted).toEqual([
      expect.objectContaining({
        sourceType: "browser_file",
        kind: "image",
        name: "photo.PNG",
        mimeType: "image/png",
        size: 1024,
      }),
      expect.objectContaining({
        sourceType: "browser_file",
        kind: "audio",
        name: "voice.mp3",
        mimeType: "audio/mpeg",
        size: 2048,
      }),
      expect.objectContaining({
        sourceType: "browser_file",
        kind: "video",
        name: "clip.mov",
        mimeType: "video/quicktime",
        size: 4096,
      }),
      expect.objectContaining({
        sourceType: "browser_file",
        kind: "document",
        name: "report.pdf",
        mimeType: "application/pdf",
        size: 8192,
      }),
      expect.objectContaining({
        sourceType: "browser_file",
        kind: "document",
        name: "budget.xlsx",
        mimeType: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        size: 4096,
      }),
    ]);
  });

  test("resolves canonical MIME types for extension-matched files with empty or conflicting file types", () => {
    const result = normalizeBrowserFileAttachmentDrafts([
      { sourceType: "browser_file", file: { name: "slides.pdf", type: "", size: 1024 } },
      { sourceType: "browser_file", file: { name: "notes.txt", type: "image/png", size: 1024 } },
    ]);

    expect(result.accepted).toEqual([
      expect.objectContaining({
        kind: "document",
        name: "slides.pdf",
        mimeType: "application/pdf",
      }),
      expect.objectContaining({
        kind: "image",
        name: "notes.txt",
        mimeType: "image/png",
      }),
    ]);
    expect(result.rejected).toEqual([]);
  });

  test("separates unsupported source types from unrecognized file types", () => {
    const result = normalizeBrowserFileAttachmentDrafts([
      { sourceType: "remote_url" },
      { sourceType: "browser_file", file: { name: "archive.zip", type: "application/zip", size: 1024 } },
    ]);

    expect(result.accepted).toEqual([]);
    expect(result.rejected).toEqual([
      expect.objectContaining({
        reason: "unsupported_source_type",
        message: expect.stringContaining("remote_url"),
      }),
      expect.objectContaining({
        reason: "unrecognized_file_type",
        message: expect.stringContaining("archive.zip"),
      }),
    ]);
  });

  test("rejects image files once their cumulative size exceeds the policy budget", () => {
    const policy = buildPolicy();
    policy.kinds.image = {
      ...policy.kinds.image,
      maxCount: 3,
      maxTotalSizeBytes: 10 * 1024 * 1024,
    };

    const result = normalizeBrowserFileAttachmentDrafts(
      [
        { sourceType: "browser_file", file: { name: "first.png", type: "image/png", size: 4 * 1024 * 1024 } },
        { sourceType: "browser_file", file: { name: "second.png", type: "image/png", size: 4 * 1024 * 1024 } },
        { sourceType: "browser_file", file: { name: "third.png", type: "image/png", size: 4 * 1024 * 1024 } },
      ],
      policy,
    );

    expect(result.accepted).toHaveLength(2);
    expect(result.rejected).toEqual([
      expect.objectContaining({
        kind: "image",
        reason: "total_size_exceeded",
        message: "图片附件总大小超过 10MB 限制",
      }),
    ]);
  });
});
