import { mkdtemp, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";

const SUPPORTED_MODES = new Set(["all", "dup"]);
const DUPLICATE_TARGETS = [
  path.join("apps", "runtime"),
  "packages",
  "scripts",
];

function normalizePath(candidate, rootDir) {
  if (!candidate) {
    return null;
  }

  const normalized = String(candidate).replaceAll("\\", "/");
  if (!path.isAbsolute(normalized)) {
    return normalized.replace(/^\.\//u, "");
  }

  return path.relative(rootDir, normalized).replaceAll("\\", "/");
}

function parseJscpdReport(reportJson, rootDir) {
  const duplicates = Array.isArray(reportJson?.duplicates) ? reportJson.duplicates : [];

  return duplicates
    .map((entry) => {
      const firstPath = normalizePath(
        entry?.firstFile?.name ?? entry?.firstFile?.path ?? entry?.fragment?.firstFile?.name,
        rootDir,
      );
      const secondPath = normalizePath(
        entry?.secondFile?.name ?? entry?.secondFile?.path ?? entry?.fragment?.secondFile?.name,
        rootDir,
      );
      const lines = entry?.lines ?? entry?.fragment?.lines ?? entry?.fragment?.lineCount ?? null;
      const tokens = entry?.tokens ?? entry?.fragment?.tokens ?? null;

      return {
        category: "duplicate-implementations",
        confidence: "likely",
        action: "review-first",
        source: [firstPath, secondPath].filter(Boolean).join(" <-> ") || null,
        detail: [
          Number.isFinite(lines) ? `${lines} duplicated lines` : null,
          Number.isFinite(tokens) ? `${tokens} duplicated tokens` : null,
        ].filter(Boolean).join(", "),
      };
    })
    .filter((finding) => finding.source);
}

async function runJscpdCommand({ cwd, outputDir }) {
  const args = [
    "--config.store-dir=.pnpm-store-local",
    "dlx",
    "jscpd",
    ...DUPLICATE_TARGETS,
    "--format",
    "typescript,javascript,rust",
    "--pattern",
    "**/*.{ts,tsx,js,mjs,cjs,rs}",
    "--gitignore",
    "--noSymlinks",
    "--ignore",
    "**/node_modules/**,**/dist/**,**/.git/**,**/coverage/**,**/build/**,**/.build/**,**/.artifacts/**,**/target/**",
    "--min-lines",
    "12",
    "--min-tokens",
    "80",
    "--reporters",
    "json",
    "--output",
    outputDir,
  ];

  return new Promise((resolve) => {
    const child = spawn("pnpm", args, {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
      shell: process.platform === "win32",
    });

    let stdout = "";
    let stderr = "";

    child.stdout.on("data", (chunk) => {
      stdout += String(chunk);
    });
    child.stderr.on("data", (chunk) => {
      stderr += String(chunk);
    });

    child.on("error", (error) => {
      resolve({
        stdout,
        stderr: stderr || String(error),
        exitCode: 1,
      });
    });

    child.on("close", (exitCode) => {
      resolve({
        stdout,
        stderr,
        exitCode: exitCode ?? 1,
      });
    });
  });
}

export async function collectDuplicateSignals(options = {}) {
  const mode = options.mode ?? "all";
  if (!SUPPORTED_MODES.has(mode)) {
    return [];
  }

  const rootDir = path.resolve(options.rootDir ?? process.cwd());
  const runCommand = options.runCommand ?? runJscpdCommand;
  const tempDir = await mkdtemp(path.join(os.tmpdir(), "workclaw-jscpd-"));
  const outputDir = path.join(tempDir, "report");

  try {
    await runCommand({ cwd: rootDir, outputDir });
    const reportPath = path.join(outputDir, "jscpd-report.json");
    const report = JSON.parse(await readFile(reportPath, "utf8"));
    return parseJscpdReport(report, rootDir);
  } catch {
    return [];
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

export {
  DUPLICATE_TARGETS,
  parseJscpdReport,
  runJscpdCommand,
};
