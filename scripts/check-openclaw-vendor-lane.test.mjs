import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = resolve(__dirname, "..");
const vendorRoot = resolve(repoRoot, "apps/runtime/sidecar/vendor/openclaw-im-core");

test("openclaw im vendor lane metadata exists", () => {
  const readmePath = resolve(vendorRoot, "README.md");
  const commitPath = resolve(vendorRoot, "UPSTREAM_COMMIT");
  const patchesPath = resolve(vendorRoot, "PATCHES.md");
  const syncScriptPath = resolve(repoRoot, "scripts/sync-openclaw-im-core.mjs");

  assert.equal(existsSync(syncScriptPath), true, "sync-openclaw-im-core.mjs should exist");
  assert.equal(existsSync(readmePath), true, "README.md should exist");
  assert.equal(existsSync(commitPath), true, "UPSTREAM_COMMIT should exist");
  assert.equal(existsSync(patchesPath), true, "PATCHES.md should exist");
  assert.match(readFileSync(commitPath, "utf8"), /\S/, "UPSTREAM_COMMIT should not be empty");
  assert.match(readFileSync(patchesPath, "utf8"), /# Local Patches/, "PATCHES.md should describe local patch policy");
});
