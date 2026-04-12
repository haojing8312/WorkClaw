import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

const SUPPORTED_MODES = new Set(["all", "loc"]);
const SKIP_SEGMENTS = new Set(["node_modules", "dist", "coverage", ".git", ".worktrees", ".tmp", "__tests__", "target"]);
const SURFACES = [
  {
    kind: "frontend",
    root: path.join("apps", "runtime", "src"),
    extensions: new Set([".ts", ".tsx"]),
    include: (name) => !/\.(test|spec)\.(ts|tsx)$/u.test(name),
    warn: 300,
    plan: 500,
  },
  {
    kind: "rust",
    root: path.join("apps", "runtime", "src-tauri"),
    extensions: new Set([".rs"]),
    include: () => true,
    warn: 500,
    plan: 800,
  },
];

function countLines(source) {
  if (!source) {
    return 0;
  }

  return source.split(/\r?\n/u).length;
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

async function walkFiles(rootPath, surface, currentRelative = "") {
  const currentPath = path.join(rootPath, currentRelative);
  let entries;
  try {
    entries = await readdir(currentPath, { withFileTypes: true });
  } catch {
    return [];
  }

  const files = [];

  for (const entry of entries) {
    if (SKIP_SEGMENTS.has(entry.name)) {
      continue;
    }

    const relativePath = path.join(currentRelative, entry.name);
    if (entry.isDirectory()) {
      files.push(...await walkFiles(rootPath, surface, relativePath));
      continue;
    }

    const extension = path.extname(entry.name);
    if (entry.isFile() && surface.extensions.has(extension) && surface.include(entry.name)) {
      files.push(relativePath);
    }
  }

  return files;
}

export async function collectLocSignals(options = {}) {
  const mode = options.mode ?? "all";
  if (!SUPPORTED_MODES.has(mode)) {
    return [];
  }

  const rootDir = path.resolve(options.rootDir ?? process.cwd());
  const findings = [];

  for (const surface of SURFACES) {
    const surfaceRoot = path.join(rootDir, surface.root);
    const files = await walkFiles(surfaceRoot, surface);

    for (const relativePath of files) {
      const absolutePath = path.join(surfaceRoot, relativePath);
      const source = await readFile(absolutePath, "utf8");
      const lines = countLines(source);
      const level = classifyLineCount(lines, surface);
      if (level === "ok") {
        continue;
      }

      const threshold = level === "plan" ? surface.plan : surface.warn;
      findings.push({
        category: "oversized-file",
        confidence: level === "plan" ? "likely" : "probable",
        action: "review-first",
        source: path.join(surface.root, relativePath).replaceAll("\\", "/"),
        detail: `${surface.kind} file has ${lines} lines (${level} threshold ${threshold}+)`,
      });
    }
  }

  return findings.sort((left, right) => left.source.localeCompare(right.source));
}

export {
  SURFACES,
  classifyLineCount,
  countLines,
};
