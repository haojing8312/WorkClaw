import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const cargoTomlPath = path.join(projectRoot, "apps", "runtime", "src-tauri", "Cargo.toml");

function readCargoToml() {
  return readFileSync(cargoTomlPath, "utf8");
}

test("desktop runtime package declares runtime as the default binary", () => {
  const cargoToml = readCargoToml();

  assert.match(
    cargoToml,
    /^\[package\][\s\S]*?^default-run\s*=\s*"runtime"\s*$/m,
    "Expected Cargo package metadata to pin runtime as the default runnable binary for desktop packaging",
  );
});

test("desktop runtime package declares explicit runtime and agent_eval binary targets", () => {
  const cargoToml = readCargoToml();

  assert.match(
    cargoToml,
    /\[\[bin\]\][\s\S]*?name\s*=\s*"runtime"[\s\S]*?path\s*=\s*"src\/main\.rs"/m,
    "Expected Cargo.toml to declare the desktop runtime binary explicitly",
  );
  assert.match(
    cargoToml,
    /\[\[bin\]\][\s\S]*?name\s*=\s*"agent_eval"[\s\S]*?path\s*=\s*"src\/bin\/agent_eval\.rs"[\s\S]*?required-features\s*=\s*\["headless-evals"\]/m,
    "Expected Cargo.toml to declare the eval harness binary explicitly without making it the packaged app entrypoint",
  );
});
