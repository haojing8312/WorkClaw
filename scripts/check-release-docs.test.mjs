import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const projectRoot = process.cwd();
const readmeZhPath = path.join(projectRoot, "README.md");
const readmeEnPath = path.join(projectRoot, "README.en.md");
const releaseNotesPath = path.join(projectRoot, ".github", "release-desktop-notes.md");
const releaseWorkflowPath = path.join(projectRoot, ".github", "workflows", "release-desktop.yml");

function readText(filePath) {
  return readFileSync(filePath, "utf8");
}

test("Chinese release guidance explains desktop package choices", () => {
  const readme = readText(readmeZhPath);

  assert.match(readme, /\.exe/, "Expected Chinese README to mention .exe downloads");
  assert.match(readme, /推荐/, "Expected Chinese README to mark the recommended installer");
  assert.match(readme, /\.deb/, "Expected Chinese README to mention .deb downloads");
  assert.match(readme, /amd64/, "Expected Chinese README to mention Linux x64 packages");
  assert.match(readme, /arm64/, "Expected Chinese README to mention Linux arm64 packages");
  assert.doesNotMatch(
    readme,
    /自动更新/,
    "Expected Chinese README not to mention auto-update behavior",
  );
});

test("English release guidance explains desktop package choices", () => {
  const readme = readText(readmeEnPath);

  assert.match(readme, /\.exe/, "Expected English README to mention .exe downloads");
  assert.match(readme, /recommended/i, "Expected English README to mark the recommended installer");
  assert.match(readme, /\.deb/, "Expected English README to mention .deb downloads");
  assert.match(readme, /amd64/, "Expected English README to mention Linux x64 packages");
  assert.match(readme, /arm64/, "Expected English README to mention Linux arm64 packages");
  assert.doesNotMatch(
    readme,
    /auto-update/i,
    "Expected English README not to mention auto-update behavior",
  );
});

test("release notes template mirrors public download guidance", () => {
  const notes = readText(releaseNotesPath);

  assert.match(notes, /\.exe/, "Expected release notes template to mention .exe downloads");
  assert.match(notes, /\.deb/, "Expected release notes template to mention .deb downloads");
  assert.match(notes, /amd64/, "Expected release notes template to mention Linux x64 packages");
  assert.match(notes, /arm64/, "Expected release notes template to mention Linux arm64 packages");
  assert.doesNotMatch(
    notes,
    /auto-update/i,
    "Expected release notes template not to mention auto-update",
  );
});

test("release workflow builds Windows and Linux deb packages", () => {
  const workflow = readText(releaseWorkflowPath);

  assert.match(workflow, /windows-latest/, "Expected workflow to build Windows packages");
  assert.match(workflow, /ubuntu-22\.04\b/, "Expected workflow to build Linux x64 packages");
  assert.match(workflow, /ubuntu-22\.04-arm/, "Expected workflow to build Linux arm64 packages");
  assert.match(workflow, /--bundles nsis/, "Expected workflow to request Windows NSIS bundles");
  assert.match(workflow, /--bundles deb/, "Expected workflow to request Linux deb bundles");
  assert.match(workflow, /--bin runtime/, "Expected workflow to package only the runtime binary");
});
