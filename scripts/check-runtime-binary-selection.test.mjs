import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const cargoTomlPath = path.join(projectRoot, "apps", "runtime", "src-tauri", "Cargo.toml");
const tauriConfigPath = path.join(projectRoot, "apps", "runtime", "src-tauri", "tauri.conf.json");
const tauriConfigTemplatePath = path.join(
  projectRoot,
  "apps",
  "runtime",
  "src-tauri",
  "tauri.conf.template.json",
);

function readCargoToml() {
  return readFileSync(cargoTomlPath, "utf8");
}

test("desktop runtime package declares runtime as the default binary and disables auto bin discovery", () => {
  const cargoToml = readCargoToml();

  assert.match(
    cargoToml,
    /^\[package\][\s\S]*?^default-run\s*=\s*"runtime"\s*$/m,
    "Expected Cargo package metadata to pin runtime as the default runnable binary for desktop packaging",
  );
  assert.match(
    cargoToml,
    /^\[package\][\s\S]*?^autobins\s*=\s*false\s*$/m,
    "Expected desktop packaging to avoid auto-discovering internal utility binaries",
  );
  assert.match(
    cargoToml,
    /^\[package\][\s\S]*?^autoexamples\s*=\s*false\s*$/m,
    "Expected internal utility examples to be declared explicitly",
  );
});

test("desktop runtime package declares runtime binary and keeps eval harness out of app binaries", () => {
  const cargoToml = readCargoToml();

  assert.match(
    cargoToml,
    /\[\[bin\]\][\s\S]*?name\s*=\s*"runtime"[\s\S]*?path\s*=\s*"src\/main\.rs"/m,
    "Expected Cargo.toml to declare the desktop runtime binary explicitly",
  );
  assert.match(
    cargoToml,
    /\[\[example\]\][\s\S]*?name\s*=\s*"agent_eval"[\s\S]*?path\s*=\s*"examples\/agent_eval\.rs"[\s\S]*?required-features\s*=\s*\["headless-evals"\]/m,
    "Expected Cargo.toml to keep the eval harness available as an example without making it a packaged app binary",
  );
  const binBlocks = cargoToml
    .split(/\n(?=\[\[(?:bin|example)\]\])/)
    .filter((block) => block.startsWith("[[bin]]"));
  assert.deepEqual(
    binBlocks.map((block) => block.match(/name\s*=\s*"([^"]+)"/)?.[1]),
    ["runtime"],
    "Expected runtime to be the only desktop binary target",
  );
});

test("desktop bundle pins the packaged binary to runtime", () => {
  const tauriConfig = JSON.parse(readFileSync(tauriConfigPath, "utf8"));
  const tauriConfigTemplate = JSON.parse(readFileSync(tauriConfigTemplatePath, "utf8"));

  assert.equal(tauriConfig.mainBinaryName, "runtime");
  assert.equal(tauriConfigTemplate.mainBinaryName, "runtime");
});
