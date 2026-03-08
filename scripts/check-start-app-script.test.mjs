import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const cmdScriptPath = path.join(projectRoot, "tmp-start-app.cmd");
const vbsScriptPath = path.join(projectRoot, "tmp-start-app.vbs");

function readScript(scriptPath) {
  return readFileSync(scriptPath, "utf8");
}

test("local start cmd script derives Rust paths from environment", () => {
  const script = readScript(cmdScriptPath);

  assert.match(
    script,
    /if not defined CARGO_HOME set "CARGO_HOME=%USERPROFILE%\\\.cargo"/i,
    "Expected tmp-start-app.cmd to default CARGO_HOME from USERPROFILE",
  );
  assert.match(
    script,
    /if not defined RUSTUP_HOME set "RUSTUP_HOME=%USERPROFILE%\\\.rustup"/i,
    "Expected tmp-start-app.cmd to default RUSTUP_HOME from USERPROFILE",
  );
  assert.match(
    script,
    /set "PATH=%RUSTUP_HOME%\\toolchains\\stable-x86_64-pc-windows-msvc\\bin;%CARGO_HOME%\\bin;%PATH%"/i,
    "Expected tmp-start-app.cmd to prepend the active Rust toolchain and cargo bin to PATH",
  );
  assert.doesNotMatch(
    script,
    /C:\\Users\\36443\\\.(cargo|rustup)/i,
    "tmp-start-app.cmd should not hardcode a specific user Rust path",
  );
});

test("local start vbs script delegates to the cmd launcher", () => {
  const script = readScript(vbsScriptPath);

  assert.match(
    script,
    /tmp-start-app\.cmd/i,
    "Expected tmp-start-app.vbs to launch the cmd helper script",
  );
  assert.doesNotMatch(
    script,
    /C:\\Users\\36443\\\.(cargo|rustup)/i,
    "tmp-start-app.vbs should not hardcode a specific user Rust path",
  );
});
