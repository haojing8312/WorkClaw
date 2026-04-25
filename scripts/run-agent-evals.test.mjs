import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import {
  buildCargoArgs,
  parseAgentEvalArgs,
  resolveAgentEvalTargetDir,
  shouldReuseTarget,
} from "./run-agent-evals.mjs";

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

test("parseAgentEvalArgs enforces required scenario and optional config", () => {
  const parsed = parseAgentEvalArgs([
    "--scenario",
    "pm_weekly_summary_xietao_2026_03_30_2026_04_04",
    "--config",
    "D:/secret/config.local.yaml",
    "--reuse-target",
  ]);

  assert.equal(parsed.scenario, "pm_weekly_summary_xietao_2026_03_30_2026_04_04");
  assert.equal(parsed.config, "D:/secret/config.local.yaml");
  assert.equal(parsed.reuseTarget, true);
});

test("buildCargoArgs forwards scenario and config to rust binary", () => {
  const args = buildCargoArgs({
    scenario: "pm_weekly_summary_xietao_2026_03_30_2026_04_04",
    config: "D:/secret/config.local.yaml",
  });

  assert.deepEqual(args, [
    "run",
    "--manifest-path",
    "apps/runtime/src-tauri/Cargo.toml",
    "--features",
    "headless-evals",
    "--example",
    "agent_eval",
    "--",
    "--scenario",
    "pm_weekly_summary_xietao_2026_03_30_2026_04_04",
    "--config",
    "D:/secret/config.local.yaml",
  ]);
});

test("resolveAgentEvalTargetDir defaults to isolated and can reuse shared target", () => {
  const isolated = resolveAgentEvalTargetDir({
    cwd: "D:/code/WorkClaw",
    scenario: "workspace_image_set_vision_2026_04_25",
    env: {},
  });
  assert.match(
    isolated,
    /[/\\]\.cargo-targets[/\\]isolated[/\\]agent-eval-workspace_image_set_vision_2026_04_25-/,
  );

  assert.equal(
    resolveAgentEvalTargetDir({
      cwd: "D:/code/WorkClaw",
      scenario: "workspace_image_set_vision_2026_04_25",
      reuseTarget: true,
      env: {},
    }),
    path.join("D:/code/WorkClaw", ".cargo-targets", "workclaw"),
  );
  assert.equal(
    shouldReuseTarget({
      env: { WORKCLAW_AGENT_EVAL_REUSE_TARGET: "true" },
    }),
    true,
  );
});
