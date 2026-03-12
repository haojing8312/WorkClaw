import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const tauriConfigPath = path.join(
  projectRoot,
  "apps",
  "runtime",
  "src-tauri",
  "tauri.conf.json",
);
const cargoTomlPath = path.join(
  projectRoot,
  "apps",
  "runtime",
  "src-tauri",
  "Cargo.toml",
);
const releaseWorkflowPath = path.join(projectRoot, ".github", "workflows", "release-windows.yml");

function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, "utf8"));
}

function readText(filePath) {
  return readFileSync(filePath, "utf8");
}

test("tauri updater is configured for desktop releases", () => {
  const config = readJson(tauriConfigPath);
  const cargoToml = readText(cargoTomlPath);
  const plugins = config?.plugins ?? {};
  const updater = plugins.updater;

  assert.ok(updater, "Expected plugins.updater to be configured in tauri.conf.json");
  assert.equal(
    typeof updater.pubkey,
    "string",
    "Expected plugins.updater.pubkey to be a non-empty string",
  );
  assert.ok(updater.pubkey.trim().length > 0, "Expected plugins.updater.pubkey to be non-empty");
  assert.ok(
    Array.isArray(updater.endpoints) && updater.endpoints.length > 0,
    "Expected plugins.updater.endpoints to be configured",
  );
  assert.equal(
    config?.bundle?.createUpdaterArtifacts,
    true,
    "Expected bundle.createUpdaterArtifacts to be enabled for latest.json generation",
  );
  assert.match(
    cargoToml,
    /tauri-plugin-updater\s*=/,
    "Expected tauri-plugin-updater dependency in Cargo.toml",
  );
});

test("release workflow publishes signed updater artifacts", () => {
  const workflow = readText(releaseWorkflowPath);

  assert.match(
    workflow,
    /tauri-apps\/tauri-action@v(?:1|0\.6\.1)/,
    "Expected release workflow to use a supported tauri-action release",
  );
  assert.match(
    workflow,
    /(?:uploadUpdaterJson|includeUpdaterJson):\s*true/,
    "Expected release workflow to publish latest.json",
  );
  assert.match(
    workflow,
    /updaterJsonPreferNsis:\s*true/,
    "Expected release workflow to prefer NSIS updater metadata on Windows",
  );
  assert.match(
    workflow,
    /TAURI_SIGNING_PRIVATE_KEY:\s*\$\{\{\s*secrets\.TAURI_SIGNING_PRIVATE_KEY\s*\}\}/,
    "Expected release workflow to provide updater signing private key",
  );
  assert.match(
    workflow,
    /TAURI_SIGNING_PRIVATE_KEY_PASSWORD:\s*\$\{\{\s*secrets\.TAURI_SIGNING_PRIVATE_KEY_PASSWORD\s*\}\}/,
    "Expected release workflow to provide updater signing key password",
  );
  assert.match(
    workflow,
    /\.github\/release-windows-notes\.md/,
    "Expected release workflow to load release notes from a tracked template file",
  );
});
