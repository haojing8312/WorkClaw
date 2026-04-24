import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

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

function resolveRuntimeScriptName(mode) {
  if (mode === "dev" || mode === "build") {
    return mode;
  }

  throw new Error(`Unsupported Tauri hook mode: ${mode || "<empty>"}`);
}

function runOrThrow(command, args, { cwd, env }) {
  const result = spawnSync(
    command,
    args,
    {
      cwd,
      env,
      stdio: "inherit",
      windowsHide: false,
      shell: process.platform === "win32" && command.toLowerCase().endsWith(".cmd"),
    },
  );

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function runNodeScript(scriptPath, { cwd, env }) {
  runOrThrow(process.execPath, [scriptPath], { cwd, env });
}

function runPnpm(runner, args, { cwd, env }) {
  runOrThrow(runner.command, [...runner.args, ...args], { cwd, env });
}

function runBuildHook({ projectRoot, runtimeRoot, env, runner }) {
  runNodeScript(path.join(projectRoot, "scripts", "apply-brand.mjs"), { cwd: projectRoot, env });
  runPnpm(runner, ["--dir", "sidecar", "build"], { cwd: runtimeRoot, env });
  runNodeScript(path.join(projectRoot, "scripts", "prepare-sidecar-runtime-bundle.mjs"), {
    cwd: projectRoot,
    env,
  });
  runPnpm(runner, ["build"], { cwd: runtimeRoot, env });
}

function runDevHook({ runtimeRoot, env, runner }) {
  runPnpm(runner, ["--dir", "sidecar", "build"], { cwd: runtimeRoot, env });
  runPnpm(runner, ["dev"], { cwd: runtimeRoot, env });
}

function main() {
  const mode = resolveRuntimeScriptName(process.argv[2]?.trim());
  const runtimeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const projectRoot = path.resolve(runtimeRoot, "..", "..");
  const env = process.env;
  const runner = resolvePnpmRunner(process.env, process.platform);

  if (mode === "build") {
    runBuildHook({ projectRoot, runtimeRoot, env, runner });
    return;
  }

  runDevHook({ runtimeRoot, env, runner });
}

main();
