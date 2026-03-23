import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const DEFAULT_WARN_LINES = 500;
const DEFAULT_PLAN_LINES = 800;
const RUST_ROOT = path.join("apps", "runtime", "src-tauri");
const SKIP_SEGMENTS = new Set(["target", "node_modules", ".git", ".worktrees", ".tmp"]);

function parseThreshold(value, fallback, flagName) {
  if (value == null) {
    return fallback;
  }

  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`[rust-large-files] Invalid ${flagName} value: ${value}`);
  }
  return parsed;
}

async function walkRustFiles(rootPath, currentRelative = "") {
  const fs = await import("node:fs/promises");
  const currentPath = path.join(rootPath, currentRelative);
  const entries = await fs.readdir(currentPath, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    if (SKIP_SEGMENTS.has(entry.name)) {
      continue;
    }

    const relativePath = path.join(currentRelative, entry.name);
    if (entry.isDirectory()) {
      files.push(...await walkRustFiles(rootPath, relativePath));
      continue;
    }

    if (entry.isFile() && entry.name.endsWith(".rs")) {
      files.push(relativePath);
    }
  }

  return files;
}

function countLines(source) {
  if (source.length === 0) {
    return 0;
  }

  return source.split(/\r?\n/).length;
}

function classifyLineCount(lineCount, thresholds) {
  if (lineCount >= thresholds.plan) {
    return "plan";
  }
  if (lineCount >= thresholds.warn) {
    return "warn";
  }
  return "ok";
}

function formatRow(entry) {
  const tag = {
    plan: "PLAN",
    warn: "WARN",
    ok: "OK",
  }[entry.level];

  return `${String(entry.lines).padStart(5, " ")}  ${tag.padEnd(7, " ")}  ${entry.path}`;
}

async function main() {
  const projectRoot = process.cwd();
  const rustRoot = path.join(projectRoot, RUST_ROOT);
  const warn = parseThreshold(process.env.RUST_FILE_WARN_LINES, DEFAULT_WARN_LINES, "RUST_FILE_WARN_LINES");
  const plan = parseThreshold(process.env.RUST_FILE_PLAN_LINES, DEFAULT_PLAN_LINES, "RUST_FILE_PLAN_LINES");

  if (!(warn < plan)) {
    throw new Error("[rust-large-files] Thresholds must satisfy warn < plan.");
  }

  const files = await walkRustFiles(rustRoot);
  const rows = await Promise.all(
    files.map(async (relativePath) => {
      const absolutePath = path.join(rustRoot, relativePath);
      const source = await readFile(absolutePath, "utf8");
      const lines = countLines(source);
      return {
        lines,
        path: path.join(RUST_ROOT, relativePath).replaceAll("\\", "/"),
        level: classifyLineCount(lines, { warn, plan }),
      };
    }),
  );

  const interestingRows = rows
    .filter((row) => row.level !== "ok")
    .sort((a, b) => b.lines - a.lines || a.path.localeCompare(b.path));

  console.log(`[rust-large-files] thresholds: warn=${warn}, plan=${plan}`);

  if (interestingRows.length === 0) {
    console.log("[rust-large-files] No Rust runtime files exceeded the warning threshold.");
    return;
  }

  console.log("[rust-large-files] Files at or above warning threshold:");
  for (const row of interestingRows) {
    console.log(formatRow(row));
  }

  const summary = interestingRows.reduce(
    (acc, row) => {
      acc[row.level] += 1;
      return acc;
    },
    { warn: 0, plan: 0 },
  );

  console.log(`[rust-large-files] summary: warn=${summary.warn}, plan=${summary.plan}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
