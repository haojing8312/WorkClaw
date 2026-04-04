import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");

test("agent eval contract files exist and secrets stay local-only", () => {
  const scenarioPath = path.join(
    root,
    "agent-evals",
    "scenarios",
    "pm_weekly_summary_xietao_2026_03_30_2026_04_04.yaml",
  );
  const exampleConfigPath = path.join(
    root,
    "agent-evals",
    "config",
    "config.example.yaml",
  );
  const gitignore = fs.readFileSync(path.join(root, ".gitignore"), "utf8");
  const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));

  assert.equal(fs.existsSync(scenarioPath), true);
  assert.equal(fs.existsSync(exampleConfigPath), true);
  assert.match(gitignore, /agent-evals\/config\/config\.local\.yaml/i);
  assert.match(gitignore, /agent-evals\/local\//i);
  assert.match(gitignore, /temp\/agent-evals\//i);
  assert.equal(typeof pkg.scripts?.["eval:agent-real"], "string");
});
