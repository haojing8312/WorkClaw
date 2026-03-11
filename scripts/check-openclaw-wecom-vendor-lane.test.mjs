import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = resolve(__dirname, "..");
const vendorRoot = resolve(repoRoot, "apps/runtime/sidecar/vendor/openclaw-im-core");

test("openclaw wecom vendor lane metadata mentions wecom adoption path", () => {
  const readme = readFileSync(resolve(vendorRoot, "README.md"), "utf8");
  const patches = readFileSync(resolve(vendorRoot, "PATCHES.md"), "utf8");

  assert.match(readme, /WeCom|wecom|企业微信/, "vendor README should mention wecom adoption path");
  assert.match(patches, /WeCom|wecom|企业微信/, "PATCHES policy should mention wecom-specific patch discipline");
});
