import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execSync } from "node:child_process";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = resolve(__dirname, "..");
const upstreamRoot = resolve(
  repoRoot,
  process.env.OPENCLAW_UPSTREAM_PATH || process.env.OPENCLAW_IM_UPSTREAM_PATH || "temp/openclaw-upstream",
);
const vendorRoot = resolve(repoRoot, "apps/runtime/sidecar/vendor/openclaw-im-core");

// This lane is intentionally empty until a second channel is adopted.
// Maintainers can extend this manifest with specific upstream files once a channel is selected.
const copyManifest = [];

function ensureDir(path) {
  mkdirSync(path, { recursive: true });
}

function syncPath(relPath) {
  const src = resolve(upstreamRoot, relPath);
  const dst = resolve(vendorRoot, relPath);
  if (!existsSync(src)) {
    throw new Error(`Missing upstream path: ${relPath}`);
  }
  ensureDir(dirname(dst));
  cpSync(src, dst, { recursive: true });
}

function getUpstreamCommit() {
  return execSync("git rev-parse --short HEAD", {
    cwd: upstreamRoot,
    stdio: ["ignore", "pipe", "pipe"],
  })
    .toString()
    .trim();
}

function ensureMetadataFiles() {
  const readmePath = resolve(vendorRoot, "README.md");
  const patchFile = resolve(vendorRoot, "PATCHES.md");
  const upstreamCommitPath = resolve(vendorRoot, "UPSTREAM_COMMIT");

  if (!existsSync(readmePath)) {
    writeFileSync(
      readmePath,
      "# OpenClaw IM Core Vendor (WorkClaw)\n\nThis folder reserves the second-channel vendor lane for future OpenClaw IM adapters.\n",
      "utf8",
    );
  }
  if (!existsSync(patchFile)) {
    writeFileSync(patchFile, "# Local Patches\n\n- none\n", "utf8");
  }
  if (!existsSync(upstreamCommitPath)) {
    writeFileSync(upstreamCommitPath, "uninitialized\n", "utf8");
  }
}

function main() {
  ensureDir(vendorRoot);
  ensureMetadataFiles();

  if (!existsSync(upstreamRoot)) {
    console.log(`OpenClaw IM upstream not found: ${upstreamRoot}`);
    console.log("Metadata lane initialized only. Set OPENCLAW_IM_UPSTREAM_PATH or OPENCLAW_UPSTREAM_PATH to sync files.");
    return;
  }

  for (const relPath of copyManifest) {
    syncPath(relPath);
  }

  const commit = getUpstreamCommit();
  writeFileSync(resolve(vendorRoot, "UPSTREAM_COMMIT"), `${commit}\n`, "utf8");

  const currentPatchText = readFileSync(resolve(vendorRoot, "PATCHES.md"), "utf8");
  if (!currentPatchText.trim()) {
    writeFileSync(resolve(vendorRoot, "PATCHES.md"), "# Local Patches\n\n- none\n", "utf8");
  }

  console.log(`openclaw-im-core metadata synced from ${upstreamRoot}`);
  console.log(`upstream commit: ${commit}`);
  console.log(`copied entries: ${copyManifest.length}`);
}

main();
