import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const readmeZhPath = path.join(projectRoot, "README.md");
const readmeEnPath = path.join(projectRoot, "README.en.md");
const releaseNotesPath = path.join(projectRoot, ".github", "release-windows-notes.md");

function readText(filePath) {
  return readFileSync(filePath, "utf8");
}

test("Chinese release guidance explains installer choices", () => {
  const readme = readText(readmeZhPath);

  assert.match(readme, /\.exe/, "Expected Chinese README to mention .exe downloads");
  assert.match(readme, /推荐/, "Expected Chinese README to mark the recommended installer");
  assert.match(readme, /\.msi/, "Expected Chinese README to mention .msi downloads");
  assert.match(readme, /企业/, "Expected Chinese README to describe enterprise deployment");
  assert.doesNotMatch(
    readme,
    /自动更新/,
    "Expected Chinese README not to mention auto-update behavior",
  );
});

test("English release guidance explains installer choices", () => {
  const readme = readText(readmeEnPath);

  assert.match(readme, /\.exe/, "Expected English README to mention .exe downloads");
  assert.match(readme, /recommended/i, "Expected English README to mark the recommended installer");
  assert.match(readme, /\.msi/, "Expected English README to mention .msi downloads");
  assert.match(readme, /enterprise/i, "Expected English README to describe enterprise deployment");
  assert.doesNotMatch(
    readme,
    /auto-update/i,
    "Expected English README not to mention auto-update behavior",
  );
});

test("release notes template mirrors public download guidance", () => {
  const notes = readText(releaseNotesPath);

  assert.match(notes, /\.exe/, "Expected release notes template to mention .exe downloads");
  assert.match(notes, /\.msi/, "Expected release notes template to mention .msi downloads");
  assert.doesNotMatch(
    notes,
    /auto-update/i,
    "Expected release notes template not to mention auto-update",
  );
});
