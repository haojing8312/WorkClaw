import path from "node:path";
import { spawn } from "node:child_process";

const SUPPORTED_MODES = new Set(["all", "deadcode"]);

function normalizeSource(candidate) {
  if (!candidate) {
    return null;
  }

  return candidate.replace(/^[\-\s:]+/, "").trim() || null;
}

function parseKnipOutput(stdout) {
  return stdout
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter(Boolean)
    .filter((line) => !/^Unused\b/i.test(line))
    .filter((line) => !/^Configuration\b/i.test(line))
    .filter((line) => !/^Knip\b/i.test(line))
    .map((line) => {
      const [sourceCandidate] = line.split(/\s+/u);
      return {
        category: "dead-code",
        confidence: "probable",
        action: "review-first",
        source: normalizeSource(sourceCandidate),
        detail: line,
      };
    });
}

async function runKnipCommand({ cwd }) {
  return new Promise((resolve) => {
    const child = spawn("pnpm", ["exec", "knip", "--production", "--no-progress"], {
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

export async function collectDeadcodeSignals(options = {}) {
  const mode = options.mode ?? "all";
  if (!SUPPORTED_MODES.has(mode)) {
    return [];
  }

  const rootDir = path.resolve(options.rootDir ?? process.cwd());
  const runCommand = options.runCommand ?? runKnipCommand;
  const result = await runCommand({ cwd: rootDir });

  if ((result?.exitCode ?? 1) !== 0) {
    return [];
  }

  const stdout = result?.stdout?.trim() ?? "";
  if (!stdout) {
    return [];
  }

  return parseKnipOutput(stdout);
}
