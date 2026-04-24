import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  buildDeployCommand,
  hasRequiredBundleOutputs,
  isRetryableWindowsDeployError,
  pruneNonRuntimeBundlePaths,
  readPnpmMajorVersion,
  repairBrokenBundleLinks,
  resolvePnpmRunner,
} from "./prepare-sidecar-runtime-bundle.mjs";

test("resolvePnpmRunner prefers npm_execpath when available", () => {
  const runner = resolvePnpmRunner({
    npm_execpath: "C:\\Users\\builder\\AppData\\Roaming\\npm\\pnpm.cjs",
  }, "win32");

  assert.equal(runner.command, process.execPath);
  assert.deepEqual(runner.args, ["C:\\Users\\builder\\AppData\\Roaming\\npm\\pnpm.cjs"]);
});

test("readPnpmMajorVersion shells out through cmd wrapping for pnpm.cmd on Windows", () => {
  const spawnCalls = [];
  const major = readPnpmMajorVersion(
    { command: "pnpm.cmd", args: [] },
    {
      env: { PATH: "C:\\pnpm" },
      cwd: "D:\\code\\WorkClaw",
      platform: "win32",
      spawn(command, args, options) {
        spawnCalls.push({ command, args, options });
        return {
          status: 0,
          stdout: "10.11.0\n",
        };
      },
    },
  );

  assert.equal(major, 10);
  assert.equal(spawnCalls.length, 1);
  assert.equal(spawnCalls[0].command, "pnpm.cmd");
  assert.deepEqual(spawnCalls[0].args, ["--version"]);
  assert.equal(spawnCalls[0].options.cwd, "D:\\code\\WorkClaw");
  assert.equal(spawnCalls[0].options.shell, true);
  assert.deepEqual(spawnCalls[0].options.env, { PATH: "C:\\pnpm" });
});

test("buildDeployCommand disables bin links via environment on Windows-safe deploys", () => {
  const runner = { command: "pnpm.cmd", args: [] };
  const baseEnv = { PATH: "C:\\bin" };
  const expectedStoreDir = path.join("D:\\code\\WorkClaw", ".pnpm-store-local");

  const result = buildDeployCommand(runner, 10, "D:\\bundle", baseEnv);

  assert.equal(result.command, "pnpm.cmd");
  assert.deepEqual(result.args, [
    "--filter",
    "workclaw-runtime-sidecar",
    "deploy",
    "--prod",
    "--config.bin-links=false",
    "--store-dir",
    expectedStoreDir,
    "--legacy",
    "D:\\bundle",
  ]);
  assert.equal(result.env.npm_config_bin_links, "false");
  assert.equal(result.env.pnpm_config_bin_links, "false");
  assert.equal(result.env.npm_config_store_dir, expectedStoreDir);
  assert.equal(result.env.pnpm_config_store_dir, expectedStoreDir);
  assert.equal(result.env.NPM_CONFIG_BIN_LINKS, "false");
  assert.equal(result.env.PNPM_CONFIG_BIN_LINKS, "false");
  assert.equal(result.env.NPM_CONFIG_STORE_DIR, expectedStoreDir);
  assert.equal(result.env.PNPM_CONFIG_STORE_DIR, expectedStoreDir);
  assert.equal(result.env.PATH, "C:\\bin");
});

test("buildDeployCommand omits legacy flag for older pnpm versions", () => {
  const runner = { command: "pnpm", args: ["--dir", "apps/runtime/sidecar"] };
  const expectedStoreDir = path.join("D:\\code\\WorkClaw", ".pnpm-store-local");

  const result = buildDeployCommand(runner, 9, "/tmp/bundle", {});

  assert.deepEqual(result.args, [
    "--dir",
    "apps/runtime/sidecar",
    "--filter",
    "workclaw-runtime-sidecar",
    "deploy",
    "--prod",
    "--config.bin-links=false",
    "--store-dir",
    expectedStoreDir,
    "/tmp/bundle",
  ]);
  assert.equal(result.env.npm_config_bin_links, "false");
  assert.equal(result.env.pnpm_config_bin_links, "false");
  assert.equal(result.env.npm_config_store_dir, expectedStoreDir);
  assert.equal(result.env.pnpm_config_store_dir, expectedStoreDir);
});

test("isRetryableWindowsDeployError recognizes transient playwright bin failures on Windows", () => {
  assert.equal(
    isRetryableWindowsDeployError(
      "WARN Failed to create bin at D:\\bundle\\node_modules\\.bin\\playwright. ENOENT: no such file or directory, chmod 'D:\\bundle\\node_modules\\.bin\\playwright.ps1'\nEPERM: operation not permitted, open 'D:\\bundle\\node_modules\\.bin\\playwright.CMD'",
      "win32",
    ),
    true,
  );
});

test("isRetryableWindowsDeployError ignores unrelated failures", () => {
  assert.equal(isRetryableWindowsDeployError("ERR_PNPM_FETCH_404", "win32"), false);
});

test("isRetryableWindowsDeployError recognizes transient pnpm store stat failures on Windows", () => {
  assert.equal(
    isRetryableWindowsDeployError(
      "ERR_PNPM_UNKNOWN UNKNOWN: unknown error, stat 'E:\\\\pnpm-store\\\\v10\\\\files\\\\4e\\\\artifact'",
      "win32",
    ),
    true,
  );
});

test("pruneNonRuntimeBundlePaths removes bundled MCP SDK example trees without touching runtime files", (t) => {
  const bundleDir = mkdtempSync(path.join(os.tmpdir(), "sidecar-runtime-bundle-"));
  t.after(() => rmSync(bundleDir, { recursive: true, force: true }));

  const pnpmExamplesDir = path.join(
    bundleDir,
    "node_modules",
    ".pnpm",
    "@modelcontextprotocol+sdk@1.27.1_zod@4.3.6",
    "node_modules",
    "@modelcontextprotocol",
    "sdk",
    "dist",
    "cjs",
    "examples",
  );
  const directExamplesDir = path.join(
    bundleDir,
    "node_modules",
    "@modelcontextprotocol",
    "sdk",
    "dist",
    "esm",
    "examples",
  );
  const hoistedExamplesDir = path.join(
    bundleDir,
    "node_modules",
    ".pnpm",
    "node_modules",
    "workclaw-runtime-sidecar",
    "node_modules",
    "@modelcontextprotocol",
    "sdk",
    "dist",
    "cjs",
    "examples",
  );
  const runtimeEntry = path.join(
    bundleDir,
    "node_modules",
    ".pnpm",
    "@modelcontextprotocol+sdk@1.27.1_zod@4.3.6",
    "node_modules",
    "@modelcontextprotocol",
    "sdk",
    "dist",
    "cjs",
    "server",
    "index.js",
  );

  mkdirSync(pnpmExamplesDir, { recursive: true });
  writeFileSync(path.join(pnpmExamplesDir, "simpleOAuthClientProvider.js"), "export {};\n");
  mkdirSync(directExamplesDir, { recursive: true });
  writeFileSync(path.join(directExamplesDir, "serverWithTools.js"), "export {};\n");
  mkdirSync(hoistedExamplesDir, { recursive: true });
  writeFileSync(path.join(hoistedExamplesDir, "streamableHttpWithSseFallbackClient.js"), "export {};\n");
  mkdirSync(path.dirname(runtimeEntry), { recursive: true });
  writeFileSync(runtimeEntry, "module.exports = {};\n");

  const prunedPaths = pruneNonRuntimeBundlePaths(bundleDir);

  assert.deepEqual(prunedPaths, [directExamplesDir, hoistedExamplesDir, pnpmExamplesDir].sort());
  assert.equal(existsSync(directExamplesDir), false);
  assert.equal(existsSync(hoistedExamplesDir), false);
  assert.equal(existsSync(pnpmExamplesDir), false);
  assert.equal(existsSync(runtimeEntry), true);
});

test("pruneNonRuntimeBundlePaths is a no-op when no targeted example directories exist", (t) => {
  const bundleDir = mkdtempSync(path.join(os.tmpdir(), "sidecar-runtime-bundle-empty-"));
  t.after(() => rmSync(bundleDir, { recursive: true, force: true }));

  mkdirSync(path.join(bundleDir, "node_modules", ".pnpm"), { recursive: true });

  assert.deepEqual(pruneNonRuntimeBundlePaths(bundleDir), []);
});

test("repairBrokenBundleLinks materializes broken virtual store symlinks from workspace packages", (t) => {
  const workspaceDir = mkdtempSync(path.join(os.tmpdir(), "workclaw-workspace-"));
  const bundleDir = path.join(workspaceDir, "apps", "runtime", "src-tauri", "resources", "sidecar-runtime");
  const workspaceVirtualStore = path.join(workspaceDir, "node_modules", ".pnpm");
  const bundleVirtualStore = path.join(bundleDir, "node_modules", ".pnpm");
  const sourcePackageDir = path.join(workspaceVirtualStore, "zod@4.3.6", "node_modules", "zod");
  const brokenLinkPath = path.join(
    bundleVirtualStore,
    "@modelcontextprotocol+sdk@1.27.1_zod@4.3.6",
    "node_modules",
    "zod",
  );
  const brokenTargetPath = path.join(bundleVirtualStore, "zod@4.3.6", "node_modules", "zod");

  t.after(() => rmSync(workspaceDir, { recursive: true, force: true }));

  mkdirSync(sourcePackageDir, { recursive: true });
  writeFileSync(path.join(sourcePackageDir, "package.json"), "{\"name\":\"zod\"}\n");
  mkdirSync(path.dirname(brokenLinkPath), { recursive: true });
  symlinkSync(brokenTargetPath, brokenLinkPath, "junction");

  const repaired = repairBrokenBundleLinks(bundleDir, workspaceDir);

  assert.equal(repaired.length, 1);
  assert.equal(repaired[0].linkPath, brokenLinkPath);
  assert.equal(repaired[0].sourcePath, sourcePackageDir);
  assert.equal(lstatSync(brokenLinkPath).isSymbolicLink(), false);
  assert.equal(readFileSync(path.join(brokenLinkPath, "package.json"), "utf8"), "{\"name\":\"zod\"}\n");
});

test("hasRequiredBundleOutputs requires package, node_modules, and dist/index.js", (t) => {
  const bundleDir = mkdtempSync(path.join(os.tmpdir(), "sidecar-runtime-ready-"));
  t.after(() => rmSync(bundleDir, { recursive: true, force: true }));

  mkdirSync(path.join(bundleDir, "node_modules"), { recursive: true });
  mkdirSync(path.join(bundleDir, "dist"), { recursive: true });
  writeFileSync(path.join(bundleDir, "package.json"), "{\"name\":\"bundle\"}\n");
  writeFileSync(path.join(bundleDir, "dist", "index.js"), "console.log('ok');\n");

  assert.equal(hasRequiredBundleOutputs(bundleDir), true);

  rmSync(path.join(bundleDir, "dist", "index.js"), { force: true });
  assert.equal(hasRequiredBundleOutputs(bundleDir), false);
});
