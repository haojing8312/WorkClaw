import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

function parseBrandArg(argv) {
  const args = [...argv];
  let brandKey = "";

  for (let index = 0; index < args.length; index += 1) {
    const current = args[index];
    if (current === "--brand") {
      brandKey = args[index + 1] || "";
      index += 1;
      continue;
    }
    if (current.startsWith("--brand=")) {
      brandKey = current.slice("--brand=".length);
    }
  }

  return brandKey.trim();
}

function resolvePnpmRunner(env = process.env, platform = process.platform) {
  if (env.npm_execpath) {
    return {
      command: process.execPath,
      args: [env.npm_execpath],
    };
  }

  return {
    command: platform === "win32" ? "pnpm.cmd" : "pnpm",
    args: [],
  };
}

function runOrThrow(command, args, { cwd, env }) {
  const result = spawnSync(command, args, {
    cwd,
    env,
    encoding: "utf8",
    stdio: "inherit",
    windowsHide: false,
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function main() {
  const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const explicitBrandKey = parseBrandArg(process.argv.slice(2));
  const env = { ...process.env };

  if (explicitBrandKey) {
    env.WORKCLAW_BRAND = explicitBrandKey;
  }

  const runner = resolvePnpmRunner(env, process.platform);
  const applyBrandArgs = [path.join(projectRoot, "scripts", "apply-brand.mjs")];
  if (explicitBrandKey) {
    applyBrandArgs.push("--brand", explicitBrandKey);
  }

  runOrThrow(process.execPath, applyBrandArgs, { cwd: projectRoot, env });
  runOrThrow(runner.command, [...runner.args, "--dir", "apps/runtime/sidecar", "build"], {
    cwd: projectRoot,
    env,
  });
  runOrThrow(runner.command, [...runner.args, "--filter", "runtime", "tauri", "build", "--no-sign"], {
    cwd: projectRoot,
    env,
  });
}

main();
