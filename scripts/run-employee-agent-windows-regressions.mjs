import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const COMMANDS = [
  [
    "test",
    "--manifest-path",
    "apps/runtime/src-tauri/Cargo.toml",
    "--test",
    "test_im_employee_agents",
    "--",
    "--list",
  ],
  [
    "run",
    "--manifest-path",
    "apps/runtime/src-tauri/Cargo.toml",
    "--example",
    "employee_group_run_regression",
  ],
  [
    "run",
    "--manifest-path",
    "apps/runtime/src-tauri/Cargo.toml",
    "--example",
    "employee_im_heavy_regression",
  ],
];

function main() {
  for (const args of COMMANDS) {
    const result = spawnSync("cargo", args, {
      stdio: "inherit",
      windowsHide: false,
      env: process.env,
    });

    if (result.status !== 0) {
      process.exit(result.status ?? 1);
    }
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
