import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const FRONTEND_COMMANDS = [
  {
    label: "wecom-settings-vitest",
    command: "pnpm",
    args: [
      "--dir",
      "apps/runtime",
      "exec",
      "vitest",
      "run",
      "src/components/__tests__/SettingsView.wecom-connector.test.tsx",
    ],
  },
];

const COMPILE_COMMANDS = [
  {
    label: "runtime-cargo-check",
    command: "cargo",
    args: ["check", "-p", "runtime"],
    cwd: "apps/runtime/src-tauri",
  },
  {
    label: "rust-fast",
    command: "pnpm",
    args: ["test:rust-fast"],
  },
];

const LIBTEST_COMMANDS = [
  {
    label: "wecom-ask-user-waiting-order",
    command: "cargo",
    args: [
      "test",
      "--manifest-path",
      "apps/runtime/src-tauri/Cargo.toml",
      "--lib",
      "maybe_notify_registered_ask_user_routes_wecom_session_via_unified_host",
      "--",
      "--nocapture",
    ],
  },
  {
    label: "wecom-approval-waiting-order",
    command: "cargo",
    args: [
      "test",
      "--manifest-path",
      "apps/runtime/src-tauri/Cargo.toml",
      "--lib",
      "maybe_notify_registered_approval_requested_routes_wecom_session_via_unified_host",
      "--",
      "--nocapture",
    ],
  },
  {
    label: "wecom-resumed-lifecycle-routing",
    command: "cargo",
    args: [
      "test",
      "--manifest-path",
      "apps/runtime/src-tauri/Cargo.toml",
      "--lib",
      "host_lifecycle_emit_routes_answer_and_resume_phases_to_wecom_host",
      "--",
      "--nocapture",
    ],
  },
  {
    label: "wecom-final-reply-dispatch",
    command: "cargo",
    args: [
      "test",
      "--manifest-path",
      "apps/runtime/src-tauri/Cargo.toml",
      "--lib",
      "host_reply_dispatch_routes_wecom_session_via_unified_host",
      "--",
      "--nocapture",
    ],
  },
];

function hasFlag(flag) {
  return process.argv.includes(flag);
}

function resolveInvocation(command, args) {
  if (command === "pnpm" && process.env.npm_execpath) {
    return {
      command: process.execPath,
      args: [process.env.npm_execpath, ...args],
      shell: false,
    };
  }
  return {
    command,
    args,
    shell: process.platform === "win32" && command.endsWith(".cmd"),
  };
}

function runCommand(step) {
  const invocation = resolveInvocation(step.command, step.args);
  const location = step.cwd ? ` (cwd=${step.cwd})` : "";
  console.error(`\n[phase3-verify] ${step.label}${location}`);
  console.error(`[phase3-verify] ${invocation.command} ${invocation.args.join(" ")}`);
  const result = spawnSync(invocation.command, invocation.args, {
    stdio: "inherit",
    windowsHide: false,
    env: process.env,
    cwd: step.cwd,
    shell: invocation.shell,
  });
  if (result.error) {
    console.error(`[phase3-verify] failed to start ${invocation.command}: ${result.error.message}`);
  }
  return result.status ?? 1;
}

function main() {
  const compileOnly = hasFlag("--compile-only");
  const steps = [...FRONTEND_COMMANDS, ...COMPILE_COMMANDS, ...(compileOnly ? [] : LIBTEST_COMMANDS)];

  console.error(
    `[phase3-verify] mode=${compileOnly ? "compile-only" : "full"} ` +
      "(full mode expects a machine that can execute runtime libtests)",
  );

  for (const step of steps) {
    const status = runCommand(step);
    if (status !== 0) {
      process.exit(status);
    }
  }

  console.error("\n[phase3-verify] all requested checks passed");
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
