import { describe, expect, test } from "vitest";
import { DEFAULT_ATTACHMENT_POLICY, buildFileInputAccept } from "../attachmentPolicy";

describe("DEFAULT_ATTACHMENT_POLICY", () => {
  test("keeps the current frontend attachment limits and file-type buckets", () => {
    expect(DEFAULT_ATTACHMENT_POLICY.maxFiles).toBe(5);
    expect(DEFAULT_ATTACHMENT_POLICY.kinds.image.enabled).toBe(true);
    expect(DEFAULT_ATTACHMENT_POLICY.kinds.image.maxCount).toBe(3);
    expect(DEFAULT_ATTACHMENT_POLICY.kinds.image.maxTotalSizeBytes).toBe(10 * 1024 * 1024);
    expect(DEFAULT_ATTACHMENT_POLICY.kinds.audio.enabled).toBe(true);
    expect(DEFAULT_ATTACHMENT_POLICY.kinds.audio.maxCount).toBe(2);
    expect(DEFAULT_ATTACHMENT_POLICY.kinds.video.enabled).toBe(true);
    expect(DEFAULT_ATTACHMENT_POLICY.kinds.video.maxCount).toBe(1);
    expect(DEFAULT_ATTACHMENT_POLICY.kinds.document.enabled).toBe(true);
    expect(DEFAULT_ATTACHMENT_POLICY.kinds.document.fileTypes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          mimeTypes: expect.arrayContaining(["text/plain", "text/markdown"]),
          extensions: expect.arrayContaining(["txt", "md", "json", "csv"]),
          maxSizeBytes: 20 * 1024 * 1024,
        }),
        expect.objectContaining({
          mimeTypes: ["application/pdf"],
          extensions: ["pdf"],
          maxSizeBytes: 20 * 1024 * 1024,
        }),
        expect.objectContaining({
          extensions: ["doc", "docx", "xls", "xlsx"],
        }),
      ]),
    );
  });
});

describe("buildFileInputAccept", () => {
  test("filters disabled kinds and removes duplicate accept tokens", () => {
    const accept = buildFileInputAccept({
      maxFiles: 2,
      kinds: {
        image: {
          enabled: true,
          maxCount: 1,
          fileTypes: [
            {
              mimeTypes: ["image/*", "image/png"],
              extensions: ["png", "jpg"],
              maxSizeBytes: 1,
            },
          ],
        },
        audio: {
          enabled: true,
          fileTypes: [
            {
              mimeTypes: ["audio/*"],
              extensions: ["mp3"],
              maxSizeBytes: 1,
            },
          ],
        },
        video: {
          enabled: false,
          fileTypes: [
            {
              mimeTypes: ["video/*"],
              extensions: ["mp4"],
              maxSizeBytes: 1,
            },
          ],
        },
        document: {
          enabled: true,
          fileTypes: [
            {
              mimeTypes: ["application/pdf", "text/plain"],
              extensions: ["pdf", "txt"],
              maxSizeBytes: 1,
            },
          ],
        },
      },
    });

    const tokens = accept.split(",");

    expect(tokens).toEqual(
      expect.arrayContaining(["image/*", "image/png", ".png", ".jpg", "audio/*", ".mp3", "application/pdf", "text/plain", ".pdf", ".txt"]),
    );
    expect(tokens).not.toEqual(expect.arrayContaining(["video/*", ".mp4"]));
    expect(new Set(tokens).size).toBe(tokens.length);
  });

  test("includes binary office document extensions in the default accept list", () => {
    const accept = buildFileInputAccept(DEFAULT_ATTACHMENT_POLICY);

    expect(accept).toContain(".xlsx");
    expect(accept).toContain(".docx");
  });
});
