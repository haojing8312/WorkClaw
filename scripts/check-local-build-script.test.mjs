import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const packageJsonPath = path.join(projectRoot, "package.json");
const buildRuntimeScriptPath = path.join(projectRoot, "scripts", "build-runtime.mjs");
const runtimePackageJsonPath = path.join(projectRoot, "apps", "runtime", "package.json");
const runtimePlaywrightConfigPath = path.join(projectRoot, "apps", "runtime", "playwright.config.ts");
const tauriConfigPath = path.join(projectRoot, "apps", "runtime", "src-tauri", "tauri.conf.json");
const tauriConfigTemplatePath = path.join(projectRoot, "apps", "runtime", "src-tauri", "tauri.conf.template.json");
const tauriHookScriptPath = path.join(projectRoot, "apps", "runtime", "scripts", "run-tauri-hook.mjs");
const runtimePlaywrightServerScriptPath = path.join(projectRoot, "apps", "runtime", "scripts", "run-playwright-web-server.mjs");

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

test("tauri hooks use the local wrapper instead of invoking pnpm directly", () => {
  const runtimePkg = JSON.parse(readFileSync(runtimePackageJsonPath, "utf8"));
  const tauriConfig = JSON.parse(readFileSync(tauriConfigPath, "utf8"));
  const tauriConfigTemplate = JSON.parse(readFileSync(tauriConfigTemplatePath, "utf8"));
  const expectedDevCommand = "node scripts/run-tauri-hook.mjs dev";
  const expectedBuildCommand = "node scripts/run-tauri-hook.mjs build";

  assert.equal(
    runtimePkg.scripts?.["dev:tauri"],
    expectedDevCommand,
    "Expected runtime dev:tauri script to use the local hook wrapper",
  );
  assert.equal(
    runtimePkg.scripts?.["build:tauri"],
    expectedBuildCommand,
    "Expected runtime build:tauri script to use the local hook wrapper",
  );

  assert.equal(
    tauriConfig.build.beforeDevCommand,
    expectedDevCommand,
    "Expected tauri.conf.json to use the local dev hook wrapper",
  );
  assert.equal(
    tauriConfig.build.beforeBuildCommand,
    expectedBuildCommand,
    "Expected tauri.conf.json to use the local build hook wrapper",
  );
  assert.equal(
    tauriConfigTemplate.build.beforeDevCommand,
    expectedDevCommand,
    "Expected tauri.conf.template.json to use the local dev hook wrapper",
  );
  assert.equal(
    tauriConfigTemplate.build.beforeBuildCommand,
    expectedBuildCommand,
    "Expected tauri.conf.template.json to use the local build hook wrapper",
  );

  const tauriHookScript = readFileSync(tauriHookScriptPath, "utf8");
  assert.match(tauriHookScript, /resolvePnpmRunner/, "Expected local Tauri hook wrapper to resolve pnpm robustly");
  assert.match(
    tauriHookScript,
    /endsWith\("\.cmd"\)/,
    "Expected local Tauri hook wrapper to invoke Windows pnpm.cmd through a shell",
  );
});

test("runtime E2E web server uses the local wrapper instead of invoking pnpm directly", () => {
  const rootPkg = JSON.parse(readFileSync(packageJsonPath, "utf8"));
  const playwrightConfig = readFileSync(runtimePlaywrightConfigPath, "utf8");
  const playwrightServerScript = readFileSync(runtimePlaywrightServerScriptPath, "utf8");

  assert.equal(
    rootPkg.scripts?.["test:e2e:runtime"],
    "node scripts/run-runtime-e2e.mjs",
    "Expected root test:e2e:runtime script to use the runtime E2E wrapper",
  );
  assert.match(
    playwrightConfig,
    /command:\s*"node scripts\/run-playwright-web-server\.mjs"/,
    "Expected Playwright webServer to use the local wrapper script",
  );
  assert.doesNotMatch(
    playwrightConfig,
    /pnpm exec vite/,
    "Expected Playwright webServer config to stop invoking pnpm directly",
  );
  assert.match(
    playwrightServerScript,
    /resolvePnpmRunner/,
    "Expected Playwright web server wrapper to resolve pnpm robustly",
  );
});
