import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import os from "node:os";
import { mkdtempSync, readFileSync } from "node:fs";
import { buildWindowsNativeHostLauncher, buildNativeHostManifest, resolveNativeHostManifestPath, writeWindowsNativeHostInstallation } from "./install-chrome-native-host.mjs";

test("buildNativeHostManifest creates a Chrome native messaging manifest", () => {
  const manifest = buildNativeHostManifest({ hostName: "dev.workclaw.runtime", command: "C:\\WorkClaw\\native-host.cmd", extensionOrigins: ["chrome-extension://abcdefghijklmnop/"] });
  assert.deepEqual(manifest, { name: "dev.workclaw.runtime", description: "WorkClaw browser bridge native host", path: "C:\\WorkClaw\\native-host.cmd", type: "stdio", allowed_origins: ["chrome-extension://abcdefghijklmnop/"] });
});

test("resolveNativeHostManifestPath uses the Chrome native messaging host directory", () => {
  const resolved = resolveNativeHostManifestPath("C:\\Users\\tester\\AppData\\Local\\Google\\Chrome\\User Data", "dev.workclaw.runtime");
  assert.equal(resolved, path.join("C:\\Users\\tester\\AppData\\Local\\Google\\Chrome\\User Data", "NativeMessagingHosts", "dev.workclaw.runtime.json"));
});

test("buildWindowsNativeHostLauncher creates a cmd wrapper for the node native host script", () => {
  const launcher = buildWindowsNativeHostLauncher({ nodePath: "C:\\Program Files\\nodejs\\node.exe", scriptPath: "E:\\code\\yzpd\\workclaw\\scripts\\workclaw-chrome-native-host.mjs", baseUrl: "http://127.0.0.1:4312" });
  assert.match(launcher, /@echo off/i);
  assert.match(launcher, /set "WORKCLAW_BROWSER_BRIDGE_BASE_URL=http:\/\/127\.0\.0\.1:4312"/i);
  assert.match(launcher, /"C:\\Program Files\\nodejs\\node\.exe" "E:\\code\\yzpd\\workclaw\\scripts\\workclaw-chrome-native-host\.mjs"/i);
});

test("writeWindowsNativeHostInstallation writes both launcher and manifest", () => {
  const tempDir = mkdtempSync(path.join(os.tmpdir(), "workclaw-native-host-"));
  const result = writeWindowsNativeHostInstallation({ chromeUserDataDir: path.join(tempDir, "chrome"), hostName: "dev.workclaw.runtime", extensionOrigin: "chrome-extension://abcdefghijklmnop/", launcherPath: path.join(tempDir, "native-host.cmd"), nodePath: "C:\\Program Files\\nodejs\\node.exe", scriptPath: "E:\\code\\yzpd\\workclaw\\scripts\\workclaw-chrome-native-host.mjs", baseUrl: "http://127.0.0.1:4312" });
  const launcher = readFileSync(result.launcherPath, "utf8");
  const manifest = JSON.parse(readFileSync(result.manifestPath, "utf8"));
  assert.equal(result.launcherPath, path.join(tempDir, "native-host.cmd"));
  assert.equal(result.manifestPath, path.join(tempDir, "chrome", "NativeMessagingHosts", "dev.workclaw.runtime.json"));
  assert.match(launcher, /WORKCLAW_BROWSER_BRIDGE_BASE_URL=http:\/\/127\.0\.0\.1:4312/);
  assert.deepEqual(manifest, { name: "dev.workclaw.runtime", description: "WorkClaw browser bridge native host", path: path.join(tempDir, "native-host.cmd"), type: "stdio", allowed_origins: ["chrome-extension://abcdefghijklmnop/"] });
});
