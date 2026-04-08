import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const packageJsonPath = path.join(projectRoot, "package.json");
const buildRuntimeScriptPath = path.join(projectRoot, "scripts", "build-runtime.mjs");

test("local desktop build skips signing while CI handles updater signatures", () => {
  const pkg = JSON.parse(readFileSync(packageJsonPath, "utf8"));
  const buildRuntime = pkg.scripts?.["build:runtime"];
  const buildApp = pkg.scripts?.["build:app"];
  const buildRuntimeScript = readFileSync(buildRuntimeScriptPath, "utf8");

  assert.equal(typeof buildRuntime, "string", "Expected root build:runtime script");
  assert.equal(typeof buildApp, "string", "Expected root build:app script");
  assert.equal(buildRuntime, "node scripts/build-runtime.mjs", "Expected build:runtime to use the brand-aware wrapper");
  assert.equal(buildApp, "node scripts/build-runtime.mjs", "Expected build:app to use the brand-aware wrapper");
  assert.match(buildRuntimeScript, /--no-sign/, "Expected local build wrapper to skip signing for developer machines");
  assert.match(buildRuntimeScript, /WORKCLAW_BRAND/, "Expected local build wrapper to forward the selected brand");
});
