import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execSync } from "node:child_process";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = resolve(__dirname, "..");
const upstreamRoot = resolve(repoRoot, process.env.OPENCLAW_UPSTREAM_PATH || "temp/openclaw-upstream");
const vendorRoot = resolve(repoRoot, "apps/runtime/sidecar/vendor/openclaw-core");

const copyManifest = [
  "src/routing/resolve-route.ts",
  "src/routing/session-key.ts",
  "src/routing/account-id.ts",
  "src/routing/account-lookup.ts",
  "src/channels/chat-type.ts",
];

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

function ensurePatchLog() {
  const patchFile = resolve(vendorRoot, "PATCHES.md");
  if (!existsSync(patchFile)) {
    writeFileSync(patchFile, "# Local Patches\n\n- none\n", "utf8");
  }
}

function applyLocalRoutingPatches() {
  const resolveRouteFile = resolve(vendorRoot, "src/routing/resolve-route.ts");
  if (!existsSync(resolveRouteFile)) {
    return;
  }
  const text = readFileSync(resolveRouteFile, "utf8")
    .replace("../agents/agent-scope.js", "../agents/agent-scope-lite.js")
    .replace("roleIds.toSorted().join(\",\")", "[...roleIds].sort().join(\",\")");
  writeFileSync(resolveRouteFile, text, "utf8");
}
function main() {
  if (!existsSync(upstreamRoot)) {
    throw new Error(
      `Upstream OpenClaw repo not found: ${upstreamRoot}. Set OPENCLAW_UPSTREAM_PATH to a valid checkout.`,
    );
  }

  ensureDir(vendorRoot);
  for (const relPath of copyManifest) {
    syncPath(relPath);
  }
  applyLocalRoutingPatches();

  const commit = getUpstreamCommit();
  writeFileSync(resolve(vendorRoot, "UPSTREAM_COMMIT"), `${commit}\n`, "utf8");
  ensurePatchLog();

  const currentPatchText = readFileSync(resolve(vendorRoot, "PATCHES.md"), "utf8");
  if (!currentPatchText.trim()) {
    writeFileSync(resolve(vendorRoot, "PATCHES.md"), "# Local Patches\n\n- none\n", "utf8");
  }

  console.log(`openclaw-core synced from ${upstreamRoot}`);
  console.log(`upstream commit: ${commit}`);
  console.log(`copied entries: ${copyManifest.length}`);
}

main();
