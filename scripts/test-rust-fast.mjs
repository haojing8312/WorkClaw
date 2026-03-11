import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const FAST_RUST_COMMANDS = [
  ["test", "--manifest-path", "packages/runtime-skill-core/Cargo.toml", "--", "--nocapture"],
  ["test", "--manifest-path", "packages/runtime-policy/Cargo.toml", "--", "--nocapture"],
  ["test", "--manifest-path", "packages/runtime-routing-core/Cargo.toml", "--", "--nocapture"],
  ["test", "--manifest-path", "packages/runtime-executor-core/Cargo.toml", "--", "--nocapture"],
  ["test", "--manifest-path", "packages/runtime-models-app/Cargo.toml", "--", "--nocapture"],
  ["test", "--manifest-path", "packages/builtin-skill-checks/Cargo.toml", "--", "--nocapture"],
];

function main() {
  for (const args of FAST_RUST_COMMANDS) {
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
