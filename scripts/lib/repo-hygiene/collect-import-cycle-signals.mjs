import { spawn } from "node:child_process";
import path from "node:path";

const SUPPORTED_MODES = new Set(["all", "cycles"]);
const CYCLE_TARGETS = [
  path.join("apps", "runtime", "src"),
  path.join("apps", "runtime", "sidecar", "src"),
];

async function runMadgeCommand({ cwd, target }) {
  const args = [
    "--config.store-dir=.pnpm-store-local",
    "dlx",
    "madge",
    "--circular",
    "--json",
    "--extensions",
    "ts,tsx,js,mjs,cjs",
    target,
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

function normalizeCyclePath(candidate) {
  return String(candidate).replaceAll("\\", "/").replace(/^\.\//u, "");
}

function parseMadgeCycles(stdout, target) {
  const parsed = JSON.parse(stdout);
  const cycles = Array.isArray(parsed)
    ? parsed
    : Array.isArray(parsed?.circular)
      ? parsed.circular
      : [];

  return cycles.map((cycle) => {
    const steps = Array.isArray(cycle) ? cycle.map((part) => normalizeCyclePath(part)) : [];
    return {
      category: "import-cycle",
      confidence: "likely",
      action: "review-first",
      source: target.replaceAll("\\", "/"),
      detail: steps.join(" -> "),
    };
  });
}

export async function collectImportCycleSignals(options = {}) {
  const mode = options.mode ?? "all";
  if (!SUPPORTED_MODES.has(mode)) {
    return [];
  }

  const rootDir = path.resolve(options.rootDir ?? process.cwd());
  const runCommand = options.runCommand ?? runMadgeCommand;
  const findings = [];

  for (const target of CYCLE_TARGETS) {
    const result = await runCommand({ cwd: rootDir, target });
    const stdout = result?.stdout?.trim() ?? "";
    if (!stdout) {
      continue;
    }

    try {
      findings.push(...parseMadgeCycles(stdout, target));
    } catch {
      continue;
    }
  }

  return findings;
}

export {
  CYCLE_TARGETS,
  parseMadgeCycles,
  runMadgeCommand,
};
