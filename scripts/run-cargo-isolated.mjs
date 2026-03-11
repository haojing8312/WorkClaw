import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

export function sanitizeLabel(label) {
  return (label || "manual")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9-_]+/g, "-")
    .replace(/^-+|-+$/g, "") || "manual";
}

export function buildIsolatedTargetDir({
  cwd = process.cwd(),
  label = "manual",
  pid = process.pid,
  now = Date.now(),
} = {}) {
  const safeLabel = sanitizeLabel(label);
  return path.join(cwd, ".cargo-targets", "isolated", `${safeLabel}-${pid}-${now}`);
}

function main() {
  const [, , rawLabel, separator, ...cargoArgs] = process.argv;
  if (!rawLabel || separator !== "--" || cargoArgs.length === 0) {
    console.error("Usage: node scripts/run-cargo-isolated.mjs <label> -- <cargo args...>");
    process.exit(1);
  }

  const targetDir = buildIsolatedTargetDir({ label: rawLabel });
  const env = {
    ...process.env,
    CARGO_TARGET_DIR: targetDir,
  };

  console.error(`[cargo:isolated] CARGO_TARGET_DIR=${targetDir}`);
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
