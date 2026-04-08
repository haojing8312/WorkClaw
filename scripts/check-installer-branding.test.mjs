import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync, readdirSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const brandRoot = path.join(projectRoot, "branding", "brands");
const tauriConfigPath = path.join(
  projectRoot,
  "apps",
  "runtime",
  "src-tauri",
  "tauri.conf.json",
);

function readConfig() {
  return JSON.parse(readFileSync(tauriConfigPath, "utf8"));
}

function readBrandManifests() {
  return readdirSync(brandRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => {
      const manifestPath = path.join(brandRoot, entry.name, "manifest.json");
      return {
        manifestPath,
        manifest: JSON.parse(readFileSync(manifestPath, "utf8")),
      };
    });
}

function resolveActiveBrandManifest(config) {
  const manifests = readBrandManifests();
  const matches = manifests.filter(({ manifest }) => (
    manifest.productName === config?.productName &&
    manifest.bundleIdentifier === config?.identifier
  ));

  assert.equal(
    matches.length,
    1,
    `Expected exactly one brand manifest to match productName=${config?.productName} identifier=${config?.identifier}`,
  );

  return matches[0].manifest;
}

function resolveAsset(relativePath) {
  return path.join(projectRoot, "apps", "runtime", "src-tauri", relativePath);
}

test("windows installers are branded and localized for zh-CN", () => {
  const config = readConfig();
  const brandManifest = resolveActiveBrandManifest(config);
  const windows = config?.bundle?.windows;
  const nsis = windows?.nsis;
  const wix = windows?.wix;
  const resources = config?.bundle?.resources;
  const productName = config?.productName;
  const identifier = config?.identifier;

  assert.ok(nsis, "Expected bundle.windows.nsis to be configured");
  assert.equal(nsis.installerIcon, "icons/icon.ico");
  assert.deepEqual(nsis.languages, ["SimpChinese"]);
  assert.equal(nsis.displayLanguageSelector, false);
  assert.equal(nsis.headerImage, "icons/installer/nsis-header.bmp");
  assert.equal(nsis.sidebarImage, "icons/installer/nsis-sidebar.bmp");

  assert.ok(wix, "Expected bundle.windows.wix to be configured");
  assert.equal(wix.language, "zh-CN");
  assert.equal(wix.bannerPath, "icons/installer/wix-banner.bmp");
  assert.equal(wix.dialogImagePath, "icons/installer/wix-dialog.bmp");

  assert.ok(Array.isArray(resources), "Expected bundle.resources to be configured");
  assert.match(
    JSON.stringify(resources),
    /sidecar-runtime/i,
    "Expected bundle.resources to include packaged sidecar runtime assets",
  );
  assert.equal(productName, brandManifest.productName, "Expected productName to come from the active brand manifest");
  assert.equal(identifier, brandManifest.bundleIdentifier, "Expected identifier to come from the active brand manifest");
  assert.match(productName, /WorkClaw|[A-Za-z0-9]+claw/i, "Expected productName to be brand-derived");
  assert.match(identifier, /^dev\.[a-z0-9.-]+\.runtime$/, "Expected identifier to follow the brand-derived runtime pattern");

  for (const assetPath of [
    nsis.installerIcon,
    nsis.headerImage,
    nsis.sidebarImage,
    wix.bannerPath,
    wix.dialogImagePath,
  ]) {
    assert.ok(existsSync(resolveAsset(assetPath)), `Expected installer asset to exist: ${assetPath}`);
  }
});
