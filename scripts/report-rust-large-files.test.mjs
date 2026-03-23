import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const packageJsonPath = path.join(projectRoot, "package.json");
const scriptPath = path.join(projectRoot, "scripts", "report-rust-large-files.mjs");

test("package exposes a rust large file reporting script", () => {
  const pkg = JSON.parse(readFileSync(packageJsonPath, "utf8"));

  assert.equal(
    typeof pkg.scripts?.["report:rust-large-files"],
    "string",
    "Expected a root report:rust-large-files script",
  );
  assert.match(
    pkg.scripts["report:rust-large-files"],
    /report-rust-large-files\.mjs/,
    "Expected report:rust-large-files to delegate to the shared script",
  );
});

test("rust large file report script uses the documented thresholds and runtime scope", () => {
  const script = readFileSync(scriptPath, "utf8");

  assert.match(script, /DEFAULT_WARN_LINES = 500/, "Expected warn threshold of 500 lines");
  assert.match(script, /DEFAULT_PLAN_LINES = 800/, "Expected split-plan threshold of 800 lines");
  assert.match(
    script,
    /apps", "runtime", "src-tauri"/,
    "Expected script to scope reporting to the Rust runtime tree",
  );
  assert.match(script, /Thresholds must satisfy warn < plan/, "Expected threshold validation");
});
