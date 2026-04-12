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
import { collectDuplicateSignals } from "./lib/repo-hygiene/collect-duplicate-signals.mjs";
import { collectImportCycleSignals } from "./lib/repo-hygiene/collect-import-cycle-signals.mjs";
import { collectLocSignals } from "./lib/repo-hygiene/collect-loc-signals.mjs";

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
        duplicate: async () => {
          calls.push("duplicate");
          return [
            {
              category: "duplicate-implementations",
              confidence: "probable",
              action: "review-first",
            },
          ];
        },
        loc: async () => {
          calls.push("loc");
          return [
            {
              category: "oversized-file",
              confidence: "likely",
              action: "review-first",
            },
          ];
        },
        cycles: async () => {
          calls.push("cycles");
          return [
            {
              category: "import-cycle",
              confidence: "likely",
              action: "review-first",
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
        duplicate: async ({ mode }) => [
          {
            category: "duplicate-implementations",
            confidence: "likely",
            action: "review-first",
            detail: `mode=${mode}`,
          },
        ],
        loc: async ({ mode }) => [
          {
            category: "oversized-file",
            confidence: "likely",
            action: "review-first",
            detail: `mode=${mode}`,
          },
        ],
        cycles: async ({ mode }) => [
          {
            category: "import-cycle",
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
      "duplicate-implementations": 1,
      "import-cycle": 1,
      "oversized-file": 1,
      "stale-doc-or-skill-reference": 1,
      "temporary-artifacts": 1,
    });
    assert.deepEqual(
      report.findings.map((finding) => finding.category).sort(),
      [
        "dead-code",
        "duplicate-implementations",
        "import-cycle",
        "oversized-file",
        "stale-doc-or-skill-reference",
        "temporary-artifacts",
      ],
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

test("review-repo-hygiene routes duplicate collector by mode", async () => {
  const outputDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-"));
  try {
    const calls = [];
    await runRepoHygieneReview({
      outputDir,
      mode: "dup",
      collectors: {
        deadcode: async () => {
          calls.push("deadcode");
          return [];
        },
        artifacts: async () => {
          calls.push("artifacts");
          return [];
        },
        drift: async () => {
          calls.push("drift");
          return [];
        },
        duplicate: async () => {
          calls.push("duplicate");
          return [{ category: "duplicate-implementations" }];
        },
        loc: async () => {
          calls.push("loc");
          return [];
        },
        cycles: async () => {
          calls.push("cycles");
          return [];
        },
      },
    });

    assert.deepEqual(calls, ["duplicate"]);
  } finally {
    await rm(outputDir, { recursive: true, force: true });
  }
});

test("review-repo-hygiene routes loc collector by mode", async () => {
  const outputDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-"));
  try {
    const calls = [];
    await runRepoHygieneReview({
      outputDir,
      mode: "loc",
      collectors: {
        deadcode: async () => {
          calls.push("deadcode");
          return [];
        },
        artifacts: async () => {
          calls.push("artifacts");
          return [];
        },
        drift: async () => {
          calls.push("drift");
          return [];
        },
        duplicate: async () => {
          calls.push("duplicate");
          return [];
        },
        loc: async () => {
          calls.push("loc");
          return [{ category: "oversized-file" }];
        },
        cycles: async () => {
          calls.push("cycles");
          return [];
        },
      },
    });

    assert.deepEqual(calls, ["loc"]);
  } finally {
    await rm(outputDir, { recursive: true, force: true });
  }
});

test("review-repo-hygiene routes cycle collector by mode", async () => {
  const outputDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-"));
  try {
    const calls = [];
    await runRepoHygieneReview({
      outputDir,
      mode: "cycles",
      collectors: {
        deadcode: async () => {
          calls.push("deadcode");
          return [];
        },
        artifacts: async () => {
          calls.push("artifacts");
          return [];
        },
        drift: async () => {
          calls.push("drift");
          return [];
        },
        duplicate: async () => {
          calls.push("duplicate");
          return [];
        },
        loc: async () => {
          calls.push("loc");
          return [];
        },
        cycles: async () => {
          calls.push("cycles");
          return [{ category: "import-cycle" }];
        },
      },
    });

    assert.deepEqual(calls, ["cycles"]);
  } finally {
    await rm(outputDir, { recursive: true, force: true });
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

test("collect-duplicate-signals parses jscpd json reports", async () => {
  const findings = await collectDuplicateSignals({
    rootDir: projectRoot,
    mode: "dup",
    runCommand: async ({ outputDir }) => {
      await mkdir(outputDir, { recursive: true });
      await writeFile(
        path.join(outputDir, "jscpd-report.json"),
        JSON.stringify({
          duplicates: [
            {
              firstFile: { name: "apps/runtime/src/App.tsx" },
              secondFile: { name: "apps/runtime/src/scenes/Home.tsx" },
              lines: 24,
              tokens: 110,
            },
          ],
        }),
        "utf8",
      );

      return { stdout: "", stderr: "", exitCode: 0 };
    },
  });

  assert.deepEqual(findings, [
    {
      category: "duplicate-implementations",
      confidence: "likely",
      action: "review-first",
      source: "apps/runtime/src/App.tsx <-> apps/runtime/src/scenes/Home.tsx",
      detail: "24 duplicated lines, 110 duplicated tokens",
    },
  ]);
});

test("collect-duplicate-signals safely skips when jscpd report is unavailable", async () => {
  const findings = await collectDuplicateSignals({
    rootDir: projectRoot,
    mode: "dup",
    runCommand: async () => ({
      stdout: "",
      stderr: "jscpd unavailable",
      exitCode: 1,
    }),
  });

  assert.deepEqual(findings, []);
});

test("collect-loc-signals reports frontend and rust oversized files", async () => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-loc-"));
  try {
    await mkdir(path.join(rootDir, "apps", "runtime", "src"), { recursive: true });
    await mkdir(path.join(rootDir, "apps", "runtime", "src-tauri", "src"), { recursive: true });
    await writeFile(
      path.join(rootDir, "apps", "runtime", "src", "LargePage.tsx"),
      `${Array.from({ length: 320 }, () => "const x = 1;").join("\n")}\n`,
      "utf8",
    );
    await writeFile(
      path.join(rootDir, "apps", "runtime", "src-tauri", "src", "giant.rs"),
      `${Array.from({ length: 820 }, () => "fn x() {}").join("\n")}\n`,
      "utf8",
    );

    const findings = await collectLocSignals({ rootDir, mode: "loc" });

    assert.deepEqual(findings, [
      {
        category: "oversized-file",
        confidence: "likely",
        action: "review-first",
        source: "apps/runtime/src-tauri/src/giant.rs",
        detail: "rust file has 821 lines (plan threshold 800+)",
      },
      {
        category: "oversized-file",
        confidence: "probable",
        action: "review-first",
        source: "apps/runtime/src/LargePage.tsx",
        detail: "frontend file has 321 lines (warn threshold 300+)",
      },
    ]);
  } finally {
    await rm(rootDir, { recursive: true, force: true });
  }
});

test("collect-loc-signals ignores frontend test files", async () => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "repo-hygiene-loc-ignore-"));
  try {
    await mkdir(path.join(rootDir, "apps", "runtime", "src"), { recursive: true });
    await writeFile(
      path.join(rootDir, "apps", "runtime", "src", "LargePage.test.tsx"),
      `${Array.from({ length: 700 }, () => "const x = 1;").join("\n")}\n`,
      "utf8",
    );

    const findings = await collectLocSignals({ rootDir, mode: "loc" });

    assert.deepEqual(findings, []);
  } finally {
    await rm(rootDir, { recursive: true, force: true });
  }
});

test("collect-import-cycle-signals parses madge circular output", async () => {
  const findings = await collectImportCycleSignals({
    rootDir: projectRoot,
    mode: "cycles",
    runCommand: async ({ target }) => {
      if (target.includes(path.join("sidecar", "src"))) {
        return {
          stdout: "[]",
          stderr: "",
          exitCode: 0,
        };
      }

      return {
        stdout: JSON.stringify([
          [
            "apps/runtime/src/lib/a.ts",
            "apps/runtime/src/lib/b.ts",
            "apps/runtime/src/lib/a.ts",
          ],
        ]),
        stderr: "",
        exitCode: 0,
      };
    },
  });

  assert.deepEqual(findings, [
    {
      category: "import-cycle",
      confidence: "likely",
      action: "review-first",
      source: "apps/runtime/src",
      detail: "apps/runtime/src/lib/a.ts -> apps/runtime/src/lib/b.ts -> apps/runtime/src/lib/a.ts",
    },
  ]);
});

test("collect-import-cycle-signals safely skips when madge is unavailable", async () => {
  const findings = await collectImportCycleSignals({
    rootDir: projectRoot,
    mode: "cycles",
    runCommand: async () => ({
      stdout: "",
      stderr: "madge unavailable",
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
