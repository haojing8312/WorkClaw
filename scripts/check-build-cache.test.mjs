import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { readFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const projectRoot = process.cwd();
const packageJsonPath = path.join(projectRoot, "package.json");
const scriptPath = path.join(projectRoot, "scripts", "check-build-cache.mjs");
const installHooksPath = path.join(projectRoot, "scripts", "install-git-hooks.mjs");

test("package exposes build cache governance entrypoints", () => {
  const pkg = JSON.parse(readFileSync(packageJsonPath, "utf8"));

  assert.equal(typeof pkg.scripts?.prepare, "string", "Expected a root prepare script");
  assert.match(pkg.scripts.prepare, /install-git-hooks\.mjs/);

  assert.equal(
    typeof pkg.scripts?.["cache:build:check"],
    "string",
    "Expected a root cache:build:check script",
  );
  assert.match(pkg.scripts["cache:build:check"], /check-build-cache\.mjs/);

  assert.equal(
    typeof pkg.scripts?.["cache:build:clean"],
    "string",
    "Expected a root cache:build:clean script",
  );
  assert.match(pkg.scripts["cache:build:clean"], /check-build-cache\.mjs/);
});

test("build cache script exports documented thresholds and target scope", () => {
  const script = readFileSync(scriptPath, "utf8");

  assert.match(script, /DEFAULT_INCREMENTAL_MAX_GB = 20/);
  assert.match(script, /DEFAULT_INCREMENTAL_MAX_AGE_DAYS = 7/);
  assert.match(script, /DEFAULT_DEPS_MAX_GB = 40/);
  assert.match(script, /DEFAULT_ISOLATED_MAX_GB = 20/);
  assert.match(script, /DEFAULT_ISOLATED_MAX_AGE_DAYS = 3/);
  assert.match(script, /DEFAULT_ISOLATED_KEEP_LATEST = 5/);
  assert.match(script, /\.cargo-targets", "isolated"/);
  assert.match(script, /cargo-targets", "workclaw", "debug"/);
});

test("git hook installer configures repository-local hooks path", () => {
  const script = readFileSync(installHooksPath, "utf8");

  assert.match(script, /core\.hooksPath/);
  assert.match(script, /\.githooks/);
});

test("local governance run prunes stale incremental caches and preserves deps", async () => {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "workclaw-cache-"));
  try {
    const module = await import(pathToFileURL(scriptPath).href);
    const debugRoot = path.join(tempRoot, "cargo-targets", "workclaw", "debug");
    const incrementalDir = path.join(debugRoot, "incremental", "unit-a");
    const depsDir = path.join(debugRoot, "deps");
    const isolatedRoot = path.join(tempRoot, ".cargo-targets", "isolated");
    const isolatedDir = path.join(isolatedRoot, "agent-eval-old-100-1");

    await mkdir(incrementalDir, { recursive: true });
    await mkdir(depsDir, { recursive: true });
    await mkdir(isolatedDir, { recursive: true });
    await writeFile(path.join(incrementalDir, "cache.bin"), Buffer.alloc(16));
    await writeFile(path.join(depsDir, "keep.pdb"), Buffer.alloc(16));
    await writeFile(path.join(isolatedDir, "artifact.bin"), Buffer.alloc(16));

    const staleDate = new Date("2026-03-01T00:00:00.000Z");
    await module.setTreeMtime(path.join(debugRoot, "incremental"), staleDate);
    await module.setTreeMtime(isolatedDir, staleDate);

    const result = await module.runBuildCacheCheck({
      projectRoot: tempRoot,
      now: new Date("2026-04-10T00:00:00.000Z"),
      mode: "check",
      ci: false,
    });

    assert.equal(result.incremental.action, "pruned");
    assert.equal(result.deps.action, "ok");
    assert.equal(result.isolated.action, "pruned");
    await assert.rejects(readFile(path.join(incrementalDir, "cache.bin")), /ENOENT/);
    await assert.rejects(readFile(path.join(isolatedDir, "artifact.bin")), /ENOENT/);
    const depsFile = await stat(path.join(depsDir, "keep.pdb"));
    assert.ok(depsFile.isFile());
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test("ci governance run never prunes incremental caches and fails oversized deps", async () => {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "workclaw-cache-"));
  try {
    const module = await import(pathToFileURL(scriptPath).href);
    const debugRoot = path.join(tempRoot, "cargo-targets", "workclaw", "debug");
    const incrementalDir = path.join(debugRoot, "incremental", "unit-b");
    const depsDir = path.join(debugRoot, "deps");
    const isolatedRoot = path.join(tempRoot, ".cargo-targets", "isolated");
    const isolatedDir = path.join(isolatedRoot, "agent-eval-old-200-2");

    await mkdir(incrementalDir, { recursive: true });
    await mkdir(depsDir, { recursive: true });
    await mkdir(isolatedDir, { recursive: true });
    await writeFile(path.join(incrementalDir, "cache.bin"), Buffer.alloc(16));
    await writeFile(path.join(depsDir, "huge.pdb"), Buffer.alloc(16));
    await writeFile(path.join(isolatedDir, "artifact.bin"), Buffer.alloc(16));

    const staleDate = new Date("2026-03-01T00:00:00.000Z");
    await module.setTreeMtime(path.join(debugRoot, "incremental"), staleDate);
    await module.setTreeMtime(isolatedDir, staleDate);

    const result = await module.runBuildCacheCheck({
      projectRoot: tempRoot,
      now: new Date("2026-04-10T00:00:00.000Z"),
      mode: "check",
      ci: true,
      thresholds: {
        incrementalMaxAgeDays: 7,
        incrementalMaxBytes: 1024,
        isolatedMaxAgeDays: 3,
        isolatedMaxBytes: 1024,
        isolatedKeepLatest: 5,
        depsMaxBytes: 8,
      },
    });

    assert.equal(result.incremental.action, "would-prune");
    assert.equal(result.isolated.action, "would-prune");
    assert.equal(result.deps.action, "blocked");
    assert.equal(result.exitCode, 1);
    assert.ok(await readFile(path.join(incrementalDir, "cache.bin")));
    assert.ok(await readFile(path.join(isolatedDir, "artifact.bin")));
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test("local governance prunes older isolated runs beyond keep-latest policy", async () => {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "workclaw-cache-"));
  try {
    const module = await import(pathToFileURL(scriptPath).href);
    const isolatedRoot = path.join(tempRoot, ".cargo-targets", "isolated");
    const now = new Date("2026-04-10T00:00:00.000Z");

    for (let index = 0; index < 6; index += 1) {
      const dir = path.join(isolatedRoot, `agent-eval-${index}-${100 + index}-${index}`);
      await mkdir(dir, { recursive: true });
      await writeFile(path.join(dir, "artifact.bin"), Buffer.alloc(16));
      await module.setTreeMtime(dir, new Date(now.getTime() - (index + 1) * 60 * 1000));
    }

    const result = await module.runBuildCacheCheck({
      projectRoot: tempRoot,
      now,
      ci: false,
      thresholds: {
        isolatedMaxAgeDays: 99,
        isolatedMaxBytes: 1024 * 1024,
        isolatedKeepLatest: 5,
      },
    });

    assert.equal(result.isolated.action, "pruned");
    assert.equal(result.isolated.prunedCount, 1);
    await assert.rejects(
      stat(path.join(isolatedRoot, "agent-eval-5-105-5")),
      /ENOENT/,
    );
    assert.ok(await stat(path.join(isolatedRoot, "agent-eval-0-100-0")));
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test("pruned cache results report reclaimed bytes", async () => {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "workclaw-cache-"));
  try {
    const module = await import(pathToFileURL(scriptPath).href);
    const isolatedRoot = path.join(tempRoot, ".cargo-targets", "isolated");
    const isolatedDir = path.join(isolatedRoot, "agent-eval-old-300-3");

    await mkdir(isolatedDir, { recursive: true });
    await writeFile(path.join(isolatedDir, "artifact.bin"), Buffer.alloc(64));
    await module.setTreeMtime(isolatedDir, new Date("2026-03-01T00:00:00.000Z"));

    const result = await module.runBuildCacheCheck({
      projectRoot: tempRoot,
      now: new Date("2026-04-10T00:00:00.000Z"),
      ci: false,
      thresholds: {
        isolatedMaxAgeDays: 3,
        isolatedMaxBytes: 1024,
        isolatedKeepLatest: 5,
      },
    });

    assert.equal(result.isolated.action, "pruned");
    assert.equal(result.isolated.reclaimedBytes, 64);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});
