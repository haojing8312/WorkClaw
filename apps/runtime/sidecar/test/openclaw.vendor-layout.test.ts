import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const vendorRoot = resolve(process.cwd(), "vendor", "openclaw-core");

test("openclaw vendor metadata exists", () => {
  const commitPath = resolve(vendorRoot, "UPSTREAM_COMMIT");
  const patchesPath = resolve(vendorRoot, "PATCHES.md");

  assert.equal(existsSync(commitPath), true, "UPSTREAM_COMMIT should exist");
  assert.equal(existsSync(patchesPath), true, "PATCHES.md should exist");

  const commit = readFileSync(commitPath, "utf8").trim();
  assert.match(commit, /^[0-9a-f]{7,40}$/);
});

