import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { basename, extname, join, resolve } from "node:path";

const DEFAULT_OUTPUT_DIR = resolve("test", "fixtures", "wecom");

function usage() {
  console.error(
    "Usage: node scripts/sanitize-wecom-captures.mjs <input-dir> [output-dir]",
  );
}

function sanitizeValue(key, value) {
  if (typeof value !== "string") {
    return value;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return value;
  }

  switch (key) {
    case "connector_id":
      return "connector-wecom-1";
    case "corp_id":
    case "workspace_id":
      return "corp-1";
    case "agent_id":
    case "account_id":
      return "agent-1";
    case "conversation_id":
    case "thread_id":
    case "reply_target":
      return "room-1";
    case "message_id":
    case "event_id":
      return "msg-1";
    case "sender_id":
      return "user-1";
    case "sender_name":
      return "用户A";
    case "topic_id":
      return "topic-1";
    case "root_id":
      return "root-1";
    case "text":
      return "请处理这条消息";
    case "occurred_at":
    case "captured_at":
    case "delivered_at":
      return "2026-03-10T00:00:05Z";
    case "id":
      return trimmed.startsWith("topic")
        ? "topic-1"
        : trimmed.startsWith("root")
          ? "root-1"
          : trimmed.startsWith("room")
            ? "room-1"
            : trimmed.startsWith("msg")
              ? "msg-1"
              : trimmed.startsWith("agent")
                ? "agent-1"
                : trimmed.startsWith("corp")
                  ? "corp-1"
                  : trimmed.startsWith("user")
                    ? "user-1"
                    : "id-1";
    case "name":
      return "示例对象";
    default:
      return value;
  }
}

function sanitizeJson(value, currentKey = "") {
  if (Array.isArray(value)) {
    return value.map((entry) => sanitizeJson(entry, currentKey));
  }

  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [key, sanitizeJson(entry, key)]),
    );
  }

  return sanitizeValue(currentKey, value);
}

async function main() {
  const inputDir = process.argv[2] ? resolve(process.argv[2]) : null;
  const outputDir = process.argv[3]
    ? resolve(process.argv[3])
    : DEFAULT_OUTPUT_DIR;

  if (!inputDir) {
    usage();
    process.exit(1);
  }

  const entries = (await readdir(inputDir, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && extname(entry.name).toLowerCase() === ".json")
    .map((entry) => entry.name)
    .sort();

  if (entries.length === 0) {
    console.error(`[wecom:sanitize] No JSON captures found under ${inputDir}`);
    process.exit(1);
  }

  await mkdir(outputDir, { recursive: true });

  for (const entry of entries) {
    const raw = await readFile(join(inputDir, entry), "utf8");
    const parsed = JSON.parse(raw);
    const inbound = sanitizeJson(parsed.inbound_event ?? {});
    const normalized = sanitizeJson(parsed.normalized_event ?? {});
    const stem = basename(entry, ".json");

    await writeFile(
      join(outputDir, `${stem}.inbound.json`),
      `${JSON.stringify(inbound, null, 2)}\n`,
      "utf8",
    );
    await writeFile(
      join(outputDir, `${stem}.normalized.json`),
      `${JSON.stringify(normalized, null, 2)}\n`,
      "utf8",
    );
  }

  console.log(
    `[wecom:sanitize] Wrote ${entries.length * 2} sanitized fixture files to ${outputDir}`,
  );
}

main().catch((error) => {
  console.error(`[wecom:sanitize] ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
});
