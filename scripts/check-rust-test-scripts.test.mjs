import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const packageJsonPath = path.join(projectRoot, "package.json");
const isolatedRunnerPath = path.join(projectRoot, "scripts", "run-cargo-isolated.mjs");
const rustFastScriptPath = path.join(projectRoot, "scripts", "test-rust-fast.mjs");

test("root package exposes fast and isolated rust validation entrypoints", () => {
  const pkg = JSON.parse(readFileSync(packageJsonPath, "utf8"));

  assert.equal(
    typeof pkg.scripts?.["test:rust-fast"],
    "string",
    "Expected a root test:rust-fast script for lightweight Rust validation",
  );
  assert.match(
    pkg.scripts["test:rust-fast"],
    /test-rust-fast\.mjs/,
    "Expected test:rust-fast to delegate to the shared Rust validation script",
  );

  assert.equal(
    typeof pkg.scripts?.["cargo:isolated"],
    "string",
    "Expected a root cargo:isolated helper script",
  );
  assert.match(
    pkg.scripts["cargo:isolated"],
    /run-cargo-isolated\.mjs/,
    "Expected cargo:isolated to delegate to the isolated cargo helper",
  );
});

test("fast rust test script covers lightweight validation crates", () => {
  const script = readFileSync(rustFastScriptPath, "utf8");

  assert.match(script, /runtime-skill-core/, "Expected fast script to cover runtime-skill-core");
  assert.match(script, /runtime-policy/, "Expected fast script to cover runtime-policy");
  assert.match(
    script,
    /runtime-routing-core/,
    "Expected fast script to cover runtime-routing-core",
  );
  assert.match(
    script,
    /runtime-executor-core/,
    "Expected fast script to cover runtime-executor-core",
  );
  assert.match(script, /runtime-models-app/, "Expected fast script to cover runtime-models-app");
  assert.match(
    script,
    /builtin-skill-checks/,
    "Expected fast script to cover builtin-skill-checks",
  );
});

test("isolated cargo helper writes targets outside package directories", () => {
  const script = readFileSync(isolatedRunnerPath, "utf8");

  assert.match(
    script,
    /\.cargo-targets/,
    "Expected helper to place isolated target dirs under the repo-level .cargo-targets root",
  );
  assert.match(
    script,
    /process\.pid/,
    "Expected helper to include the current process id in isolated target directories",
  );
  assert.doesNotMatch(
    script,
    /src-tauri[\\/]+target-/i,
    "Isolated helper must not write temporary target dirs inside the src-tauri package",
  );
});
