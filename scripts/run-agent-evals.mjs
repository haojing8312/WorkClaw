import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { buildIsolatedTargetDir } from "./run-cargo-isolated.mjs";

export function parseAgentEvalArgs(argv) {
  const args = Array.from(argv);
  let scenario = null;
  let config = null;

  for (let index = 0; index < args.length; ) {
    const current = args[index];
    if (current === "--scenario") {
      scenario = args[index + 1] ?? null;
      index += 2;
      continue;
    }
    if (current === "--config") {
      config = args[index + 1] ?? null;
      index += 2;
      continue;
    }
    if (current === "--help" || current === "-h") {
      return { help: true, scenario: null, config: null };
    }
    throw new Error(`Unknown argument: ${current}`);
  }

  if (!scenario) {
    throw new Error("Missing required --scenario <id>");
  }

  return { help: false, scenario, config };
}

export function buildCargoArgs({ scenario, config }) {
  const cargoArgs = [
    "run",
    "--manifest-path",
    "apps/runtime/src-tauri/Cargo.toml",
    "--bin",
    "agent_eval",
    "--",
    "--scenario",
    scenario,
  ];
  if (config) {
    cargoArgs.push("--config", config);
  }
  return cargoArgs;
}

function printUsage() {
  console.error(
    "Usage: pnpm eval:agent-real --scenario <id> [--config <path-to-config.local.yaml>]",
  );
}

function main() {
  let parsed;
  try {
    parsed = parseAgentEvalArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`[agent-eval] ${error.message}`);
    printUsage();
    process.exit(1);
  }

  if (parsed.help) {
    printUsage();
    process.exit(0);
  }

  const targetDir = buildIsolatedTargetDir({
    cwd: path.resolve(import.meta.dirname, ".."),
    label: `agent-eval-${parsed.scenario}`,
  });
  const env = {
    ...process.env,
    CARGO_TARGET_DIR: targetDir,
  };
  const cargoArgs = buildCargoArgs(parsed);

  console.error(`[agent-eval] CARGO_TARGET_DIR=${targetDir}`);
  const result = spawnSync("cargo", cargoArgs, {
    stdio: "inherit",
    windowsHide: false,
    env,
  });

  process.exit(result.status ?? 1);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
