import { readFile } from "node:fs/promises";
import path from "node:path";

const refName = process.env.GITHUB_REF_NAME;

if (!refName) {
  console.error("[release] Missing GITHUB_REF_NAME environment variable.");
  process.exit(1);
}

if (!/^v\d+\.\d+\.\d+$/.test(refName)) {
  console.error(`[release] Invalid tag format: "${refName}". Expected vX.Y.Z.`);
  process.exit(1);
}

const tauriConfigPath = path.join(
  process.cwd(),
  "apps",
  "runtime",
  "src-tauri",
  "tauri.conf.json",
);

const tauriConfigRaw = await readFile(tauriConfigPath, "utf8");
const tauriConfig = JSON.parse(tauriConfigRaw);
const appVersion = tauriConfig?.version;
const tagVersion = refName.slice(1);

if (appVersion !== tagVersion) {
  console.error(
    `[release] Version mismatch: tag=${tagVersion}, tauri.conf.json=${appVersion}.`,
  );
  console.error("[release] Please update apps/runtime/src-tauri/tauri.conf.json version.");
  process.exit(1);
}

console.log(`[release] Version check passed: ${refName} matches tauri.conf.json.`);
