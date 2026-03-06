import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const packageJsonPath = path.join(projectRoot, "package.json");

test("local desktop build skips signing while CI handles updater signatures", () => {
  const pkg = JSON.parse(readFileSync(packageJsonPath, "utf8"));
  const buildRuntime = pkg.scripts?.["build:runtime"];
  const buildApp = pkg.scripts?.["build:app"];

  assert.equal(typeof buildRuntime, "string", "Expected root build:runtime script");
  assert.equal(typeof buildApp, "string", "Expected root build:app script");
  assert.match(
    buildRuntime,
    /tauri build --no-sign/,
    "Expected local build:runtime to skip signing for developer machines",
  );
  assert.match(
    buildApp,
    /tauri build --no-sign/,
    "Expected local build:app to skip signing for developer machines",
  );
});
