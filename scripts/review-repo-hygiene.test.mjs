import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { runRepoHygieneReview } from "./review-repo-hygiene.mjs";
import { collectArtifactsSignals } from "./lib/repo-hygiene/collect-artifacts-signals.mjs";
import { collectDeadcodeSignals } from "./lib/repo-hygiene/collect-deadcode-signals.mjs";
import { collectDriftSignals } from "./lib/repo-hygiene/collect-drift-signals.mjs";

const projectRoot = process.cwd();
const scriptPath = path.join(projectRoot, "scripts", "review-repo-hygiene.mjs");

test("review-repo-hygiene writes summary and json outputs for a supported mode", async () => {
  const outputDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-"));
  try {
    const proc = spawn(process.execPath, [
      scriptPath,
      "--output-dir",
      outputDir,
      "--mode",
      "artifacts",
    ], {
      cwd: projectRoot,
      stdio: "pipe",
    });

    let stderr = "";
    proc.stderr.on("data", (chunk) => {
      stderr += String(chunk);
    });

    const exitCode = await new Promise((resolve, reject) => {
      proc.on("error", reject);
      proc.on("close", resolve);
    });

    assert.equal(exitCode, 0, stderr);

    const summary = await readFile(path.join(outputDir, "summary.md"), "utf8");
    const report = JSON.parse(await readFile(path.join(outputDir, "report.json"), "utf8"));

    assert.match(summary, /Repo Hygiene Report/);
    assert.match(summary, /Mode: artifacts/);
    assert.equal(Array.isArray(report.findings), true);
    assert.equal(report.mode, "artifacts");
    assert.equal(typeof report.generatedAt, "string");
  } finally {
    await rm(outputDir, { recursive: true, force: true });
  }
});

test("review-repo-hygiene routes collectors by mode", async () => {
  const outputDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-"));
  try {
    const calls = [];
    await runRepoHygieneReview({
      outputDir,
      mode: "deadcode",
      collectors: {
        deadcode: async () => {
          calls.push("deadcode");
          return [
            {
              category: "dead-code",
              confidence: "confirmed",
              action: "ignore-with-rationale",
            },
          ];
        },
        artifacts: async () => {
          calls.push("artifacts");
          return [
            {
              category: "temporary-artifacts",
              confidence: "probable",
              action: "ignore-with-rationale",
            },
          ];
        },
        drift: async () => {
          calls.push("drift");
          return [
            {
              category: "stale-doc-or-skill-reference",
              confidence: "probable",
              action: "ignore-with-rationale",
            },
          ];
        },
      },
    });

    assert.deepEqual(calls, ["deadcode"]);
    const report = JSON.parse(await readFile(path.join(outputDir, "report.json"), "utf8"));
    assert.deepEqual(report.countsByCategory, { "dead-code": 1 });
  } finally {
    await rm(outputDir, { recursive: true, force: true });
  }
});

test("review-repo-hygiene module can be imported without CLI argv assumptions", async () => {
  const mod = await import("./review-repo-hygiene.mjs");

  assert.equal(typeof mod.runRepoHygieneReview, "function");
  assert.equal(typeof mod.parseArgs, "function");
});

test("review-repo-hygiene keeps category visibility for all mode", async () => {
  const outputDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-"));
  try {
    await runRepoHygieneReview({
      outputDir,
      mode: "all",
      collectors: {
        deadcode: async ({ mode }) => [
          {
            category: "dead-code",
            confidence: "confirmed",
            action: "ignore-with-rationale",
            detail: `mode=${mode}`,
          },
        ],
        artifacts: async ({ mode }) => [
          {
            category: "temporary-artifacts",
            confidence: "likely",
            action: "review-first",
            detail: `mode=${mode}`,
          },
        ],
        drift: async ({ mode }) => [
          {
            category: "stale-doc-or-skill-reference",
            confidence: "likely",
            action: "review-first",
            detail: `mode=${mode}`,
          },
        ],
      },
    });

    const report = JSON.parse(await readFile(path.join(outputDir, "report.json"), "utf8"));

    assert.deepEqual(report.countsByCategory, {
      "dead-code": 1,
      "stale-doc-or-skill-reference": 1,
      "temporary-artifacts": 1,
    });
    assert.deepEqual(
      report.findings.map((finding) => finding.category).sort(),
      ["dead-code", "stale-doc-or-skill-reference", "temporary-artifacts"],
    );
    assert.equal(
      report.findings.every((finding) => finding.detail === "mode=all"),
      true,
    );
  } finally {
    await rm(outputDir, { recursive: true, force: true });
  }
});

test("collect-artifacts-signals reports deterministic root temporary artifacts", async () => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-artifacts-"));
  try {
    await writeFile(path.join(rootDir, "tmp-session.log"), "artifact\n", "utf8");
    await writeFile(path.join(rootDir, "temp-runtime.txt"), "artifact\n", "utf8");
    await writeFile(path.join(rootDir, ".tmp_state"), "artifact\n", "utf8");
    await writeFile(path.join(rootDir, "release.tgz"), "artifact\n", "utf8");
    await writeFile(path.join(rootDir, "tmp-start-app.cmd"), "keep\n", "utf8");
    await writeFile(path.join(rootDir, "tmp-start-app.vbs"), "keep\n", "utf8");
    await writeFile(path.join(rootDir, "README.md"), "keep\n", "utf8");

    const findings = await collectArtifactsSignals({ rootDir, mode: "artifacts" });

    assert.equal(Array.isArray(findings), true);
    assert.equal(findings.every((finding) => finding.category === "temporary-artifacts"), true);
    assert.deepEqual(
      findings.map((finding) => finding.source),
      [".tmp_state", "release.tgz", "temp-runtime.txt", "tmp-session.log"],
    );
  } finally {
    await rm(rootDir, { recursive: true, force: true });
  }
});

test("collect-deadcode-signals degrades safely when knip exits non-zero", async () => {
  const findings = await collectDeadcodeSignals({
    rootDir: projectRoot,
    mode: "deadcode",
    runCommand: async () => ({
      stdout: "",
      stderr: "knip missing",
      exitCode: 1,
    }),
    runCargoCommand: async () => ({
      stdout: "",
      stderr: "cargo tool unavailable",
      exitCode: 1,
    }),
  });

  assert.deepEqual(findings, []);
});

test("collect-deadcode-signals shapes knip findings on success", async () => {
  const findings = await collectDeadcodeSignals({
    rootDir: projectRoot,
    mode: "deadcode",
    runCommand: async () => ({
      stdout: [
        "Unused files (2)",
        "src/unused.ts Unused file",
        "src/extra.ts Unused export",
        "",
      ].join("\n"),
      stderr: "",
      exitCode: 0,
    }),
    runCargoCommand: async () => ({
      stdout: "",
      stderr: "cargo tool unavailable",
      exitCode: 1,
    }),
  });

  assert.deepEqual(findings, [
    {
      category: "dead-code",
      confidence: "probable",
      action: "review-first",
      language: "ts",
      source: "src/unused.ts",
      detail: "src/unused.ts Unused file",
    },
    {
      category: "dead-code",
      confidence: "probable",
      action: "review-first",
      language: "ts",
      source: "src/extra.ts",
      detail: "src/extra.ts Unused export",
    },
  ]);
});

test("collect-deadcode-signals includes rust findings when cargo deadcode tool is available", async () => {
  const findings = await collectDeadcodeSignals({
    rootDir: projectRoot,
    mode: "deadcode",
    runCommand: async () => ({
      stdout: "",
      stderr: "",
      exitCode: 0,
    }),
    runCargoCommand: async ({ args }) => {
      if (args.includes("--help")) {
        if (args[0] === "machete") {
          return { stdout: "cargo machete help", stderr: "", exitCode: 0 };
        }
        return { stdout: "", stderr: "missing", exitCode: 1 };
      }

      return {
        stdout: [
          "cargo-machete found the following unused dependencies in this directory:",
          "tauri-app -- apps/runtime/src-tauri/Cargo.toml:",
          "\tserde_json",
          "runtime-policy -- packages/runtime-policy/Cargo.toml:",
          "\tregex",
        ].join("\n"),
        stderr: "",
        exitCode: 1,
      };
    },
  });

  assert.deepEqual(findings, [
    {
      category: "dead-code",
      confidence: "probable",
      action: "review-first",
      language: "rust",
      tool: "cargo-machete",
      source: "apps/runtime/src-tauri/Cargo.toml",
      detail: "unused dependency serde_json",
    },
    {
      category: "dead-code",
      confidence: "probable",
      action: "review-first",
      language: "rust",
      tool: "cargo-machete",
      source: "packages/runtime-policy/Cargo.toml",
      detail: "unused dependency regex",
    },
  ]);
});

test("collect-deadcode-signals safely skips rust detection when no cargo deadcode tool is available", async () => {
  const findings = await collectDeadcodeSignals({
    rootDir: projectRoot,
    mode: "deadcode",
    runCommand: async () => ({
      stdout: "",
      stderr: "",
      exitCode: 0,
    }),
    runCargoCommand: async () => ({
      stdout: "",
      stderr: "missing",
      exitCode: 1,
    }),
  });

  assert.deepEqual(findings, []);
});

test("collect-drift-signals reports missing repo hygiene references deterministically", async () => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-drift-"));
  try {
    await writeFile(
      path.join(rootDir, "package.json"),
      JSON.stringify(
        {
          name: "fixture",
          scripts: {
            "review:repo-hygiene": "node scripts/review-repo-hygiene.mjs",
          },
        },
        null,
        2,
      ),
      "utf8",
    );
    await mkdir(path.join(rootDir, "scripts"), { recursive: true });
    await writeFile(
      path.join(rootDir, "scripts", "review-repo-hygiene.mjs"),
      "export {};\n",
      "utf8",
    );
    await mkdir(
      path.join(rootDir, ".agents", "skills", "workclaw-repo-hygiene-review"),
      { recursive: true },
    );
    await writeFile(
      path.join(rootDir, ".agents", "skills", "workclaw-repo-hygiene-review", "SKILL.md"),
      "---\nname: workclaw-repo-hygiene-review\n---\n",
      "utf8",
    );
    await writeFile(
      path.join(rootDir, "AGENTS.md"),
      [
        "# AGENTS",
        "",
        "Use `pnpm review:repo-hygiene` first.",
        "Use `workclaw-repo-hygiene-review` before cleanup.",
      ].join("\n"),
      "utf8",
    );
    await mkdir(path.join(rootDir, "docs", "maintenance"), { recursive: true });
    await writeFile(
      path.join(rootDir, "docs", "maintenance", "repo-hygiene.md"),
      [
        "# Repo Hygiene",
        "",
        "Run `pnpm review:repo-hygiene` first.",
        "Use `workclaw-repo-hygiene-review` to classify candidates.",
        "Reports are written to `.artifacts/repo-hygiene/` for local review.",
      ].join("\n"),
      "utf8",
    );

    const findings = await collectDriftSignals({ rootDir, mode: "drift" });

    assert.deepEqual(
      findings.map((finding) => finding.category).sort(),
      ["stale-doc-or-skill-reference", "stale-doc-or-skill-reference"],
    );
    assert.equal(
      findings.some((finding) => finding.detail.includes("workclaw-cleanup-execution AGENTS reference")),
      true,
    );
    assert.equal(
      findings.some((finding) => finding.detail.includes("workclaw-cleanup-execution maintenance doc reference")),
      true,
    );
  } finally {
    await rm(rootDir, { recursive: true, force: true });
  }
});

test("collect-drift-signals reports missing referenced skill files deterministically", async () => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-drift-target-"));
  try {
    await writeFile(
      path.join(rootDir, "package.json"),
      JSON.stringify(
        {
          name: "fixture",
          scripts: {
            "review:repo-hygiene": "node scripts/review-repo-hygiene.mjs",
          },
        },
        null,
        2,
      ),
      "utf8",
    );
    await mkdir(path.join(rootDir, "scripts"), { recursive: true });
    await writeFile(
      path.join(rootDir, "scripts", "review-repo-hygiene.mjs"),
      "export {};\n",
      "utf8",
    );
    await writeFile(
      path.join(rootDir, "AGENTS.md"),
      [
        "# AGENTS",
        "",
        "Use `pnpm review:repo-hygiene` first.",
        "Use `workclaw-repo-hygiene-review` before cleanup.",
        "Use `workclaw-cleanup-execution` only after review.",
      ].join("\n"),
      "utf8",
    );
    await mkdir(path.join(rootDir, "docs", "maintenance"), { recursive: true });
    await writeFile(
      path.join(rootDir, "docs", "maintenance", "repo-hygiene.md"),
      [
        "# Repo Hygiene",
        "",
        "Run `pnpm review:repo-hygiene` first.",
        "Use `workclaw-repo-hygiene-review` to classify candidates.",
        "Use `workclaw-cleanup-execution` after review.",
        "Reports are written to `.artifacts/repo-hygiene/` for local review.",
      ].join("\n"),
      "utf8",
    );
    await mkdir(
      path.join(rootDir, ".agents", "skills", "workclaw-repo-hygiene-review"),
      { recursive: true },
    );
    await writeFile(
      path.join(rootDir, ".agents", "skills", "workclaw-repo-hygiene-review", "SKILL.md"),
      "---\nname: workclaw-repo-hygiene-review\n---\n",
      "utf8",
    );

    const findings = await collectDriftSignals({ rootDir, mode: "drift" });

    assert.deepEqual(findings, [
      {
        category: "stale-doc-or-skill-reference",
        confidence: "probable",
        action: "review-first",
        source: ".agents/skills/workclaw-cleanup-execution/SKILL.md",
        detail: "Missing workclaw-cleanup-execution skill file",
      },
    ]);
  } finally {
    await rm(rootDir, { recursive: true, force: true });
  }
});

test("collect-drift-signals stays clean when repo hygiene references and skill files are present", async () => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-drift-clean-"));
  try {
    await writeFile(
      path.join(rootDir, "package.json"),
      JSON.stringify(
        {
          name: "fixture",
          scripts: {
            "review:repo-hygiene": "node scripts/review-repo-hygiene.mjs",
          },
        },
        null,
        2,
      ),
      "utf8",
    );
    await mkdir(path.join(rootDir, "scripts"), { recursive: true });
    await writeFile(
      path.join(rootDir, "scripts", "review-repo-hygiene.mjs"),
      "export {};\n",
      "utf8",
    );
    await writeFile(
      path.join(rootDir, "AGENTS.md"),
      [
        "# AGENTS",
        "",
        "Use `pnpm review:repo-hygiene` first.",
        "Use `workclaw-repo-hygiene-review` before cleanup.",
        "Use `workclaw-cleanup-execution` only after review.",
      ].join("\n"),
      "utf8",
    );
    await mkdir(path.join(rootDir, "docs", "maintenance"), { recursive: true });
    await writeFile(
      path.join(rootDir, "docs", "maintenance", "repo-hygiene.md"),
      [
        "# Repo Hygiene",
        "",
        "Run `pnpm review:repo-hygiene` first.",
        "Use `workclaw-repo-hygiene-review` to classify candidates.",
        "Use `workclaw-cleanup-execution` after review.",
        "Reports are written to `.artifacts/repo-hygiene/` for local review.",
      ].join("\n"),
      "utf8",
    );
    await mkdir(
      path.join(rootDir, ".agents", "skills", "workclaw-repo-hygiene-review"),
      { recursive: true },
    );
    await writeFile(
      path.join(rootDir, ".agents", "skills", "workclaw-repo-hygiene-review", "SKILL.md"),
      "---\nname: workclaw-repo-hygiene-review\n---\n",
      "utf8",
    );
    await mkdir(
      path.join(rootDir, ".agents", "skills", "workclaw-cleanup-execution"),
      { recursive: true },
    );
    await writeFile(
      path.join(rootDir, ".agents", "skills", "workclaw-cleanup-execution", "SKILL.md"),
      "---\nname: workclaw-cleanup-execution\n---\n",
      "utf8",
    );

    const findings = await collectDriftSignals({ rootDir, mode: "drift" });

    assert.deepEqual(findings, []);
  } finally {
    await rm(rootDir, { recursive: true, force: true });
  }
});
