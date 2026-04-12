import path from "node:path";
import { spawn } from "node:child_process";
import os from "node:os";

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
        language: "ts",
        source: normalizeSource(sourceCandidate),
        detail: line,
      };
    });
}

function parseRustDeadcodeOutput(stdout, tool) {
  if (tool === "cargo-machete") {
    const findings = [];
    let currentManifest = null;

    for (const rawLine of stdout.split(/\r?\n/u)) {
      const line = rawLine.trim();
      if (!line) {
        continue;
      }

      const manifestMatch = line.match(/^[^\s].*--\s+(.+Cargo\.toml):$/u);
      if (manifestMatch) {
        currentManifest = manifestMatch[1].replaceAll("\\", "/");
        continue;
      }

      if (currentManifest && /^[A-Za-z0-9_.-]+$/u.test(line)) {
        findings.push({
          category: "dead-code",
          confidence: "probable",
          action: "review-first",
          language: "rust",
          tool,
          source: currentManifest,
          detail: `unused dependency ${line}`,
        });
      }
    }

    return findings;
  }

  return stdout
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter(Boolean)
    .filter((line) => !/^warning:/i.test(line))
    .filter((line) => !/^info:/i.test(line))
    .filter((line) => !/^unused/i.test(line))
    .map((line) => {
      const sourceCandidate = line.split(/\s+/u)[0];
      return {
        category: "dead-code",
        confidence: "probable",
        action: "review-first",
        language: "rust",
        tool,
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

async function runCargoCommand({ cwd, args }) {
  const cargoHome = process.env.CARGO_HOME
    || (process.platform === "win32" && "D:/worksoftdata/.cargo")
    || path.join(os.homedir(), ".cargo");
  const toolName = args[0];
  const executableName = toolName === "machete"
    ? "cargo-machete"
    : toolName === "udeps"
      ? "cargo-udeps"
      : toolName;
  const directExecutable = process.platform === "win32"
    ? path.join(cargoHome, "bin", `${executableName}.exe`)
    : path.join(cargoHome, "bin", executableName);

  const command = toolName === "machete" || toolName === "udeps"
    ? (directExecutable)
    : "cargo";
  const commandArgs = toolName === "machete" || toolName === "udeps"
    ? args.slice(1)
    : args;

  return new Promise((resolve) => {
    const child = spawn(command, commandArgs, {
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

async function collectRustDeadcodeSignals(options = {}) {
  const rootDir = path.resolve(options.rootDir ?? process.cwd());
  const runCargo = options.runCargoCommand ?? runCargoCommand;
  const rustCandidates = [
    {
      tool: "cargo-machete",
      args: ["machete"],
    },
    {
      tool: "cargo-udeps",
      args: ["udeps"],
    },
  ];

  for (const candidate of rustCandidates) {
    const result = await runCargo({ cwd: rootDir, args: [...candidate.args, "--help"] });
    if ((result?.exitCode ?? 1) !== 0) {
      continue;
    }

    const scanResult = await runCargo({ cwd: rootDir, args: candidate.args });
    const stdout = scanResult?.stdout?.trim() ?? "";
    const exitCode = scanResult?.exitCode ?? 1;
    if (!stdout && exitCode !== 0) {
      return [];
    }

    if (!stdout) {
      return [];
    }

    return parseRustDeadcodeOutput(stdout, candidate.tool);
  }

  return [];
}

export async function collectDeadcodeSignals(options = {}) {
  const mode = options.mode ?? "all";
  if (!SUPPORTED_MODES.has(mode)) {
    return [];
  }

  const rootDir = path.resolve(options.rootDir ?? process.cwd());
  const runCommand = options.runCommand ?? runKnipCommand;
  const [tsResult, rustFindings] = await Promise.all([
    runCommand({ cwd: rootDir }),
    collectRustDeadcodeSignals(options),
  ]);

  const findings = [...rustFindings];
  if ((tsResult?.exitCode ?? 1) === 0) {
    const stdout = tsResult?.stdout?.trim() ?? "";
    if (stdout) {
      findings.push(...parseKnipOutput(stdout));
    }
  }

  return findings;
}
