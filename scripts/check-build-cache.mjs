import { readdir, rm, stat, utimes } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

export const DEFAULT_INCREMENTAL_MAX_GB = 20;
export const DEFAULT_INCREMENTAL_MAX_AGE_DAYS = 7;
export const DEFAULT_DEPS_MAX_GB = 40;
export const DEFAULT_ISOLATED_MAX_GB = 20;
export const DEFAULT_ISOLATED_MAX_AGE_DAYS = 3;
export const DEFAULT_ISOLATED_KEEP_LATEST = 5;
export const DEFAULT_DEBUG_ROOT = path.join("cargo-targets", "workclaw", "debug");
export const DEFAULT_ISOLATED_ROOT = path.join(".cargo-targets", "isolated");
const MS_PER_DAY = 24 * 60 * 60 * 1000;

function bytesFromGb(value) {
  return value * 1024 * 1024 * 1024;
}

function formatBytes(bytes) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const units = ["KB", "MB", "GB", "TB"];
  let current = bytes;
  let unitIndex = -1;
  do {
    current /= 1024;
    unitIndex += 1;
  } while (current >= 1024 && unitIndex < units.length - 1);

  return `${current.toFixed(1)} ${units[unitIndex]}`;
}

async function inspectTree(targetPath) {
  try {
    const rootStats = await stat(targetPath);
    if (!rootStats.isDirectory()) {
      return { exists: false, bytes: 0, latestMtimeMs: 0 };
    }

    let bytes = 0;
    let latestMtimeMs = rootStats.mtimeMs;
    const stack = [targetPath];

    while (stack.length > 0) {
      const currentPath = stack.pop();
      const entries = await readdir(currentPath, { withFileTypes: true });
      for (const entry of entries) {
        const entryPath = path.join(currentPath, entry.name);
        const entryStats = await stat(entryPath);
        latestMtimeMs = Math.max(latestMtimeMs, entryStats.mtimeMs);

        if (entry.isDirectory()) {
          stack.push(entryPath);
          continue;
        }

        if (entry.isFile()) {
          bytes += entryStats.size;
        }
      }
    }

    return { exists: true, bytes, latestMtimeMs };
  } catch (error) {
    if (error && typeof error === "object" && "code" in error && error.code === "ENOENT") {
      return { exists: false, bytes: 0, latestMtimeMs: 0 };
    }
    throw error;
  }
}

function buildThresholds(overrides = {}) {
  return {
    incrementalMaxAgeDays: overrides.incrementalMaxAgeDays ?? DEFAULT_INCREMENTAL_MAX_AGE_DAYS,
    incrementalMaxBytes: overrides.incrementalMaxBytes ?? bytesFromGb(DEFAULT_INCREMENTAL_MAX_GB),
    depsMaxBytes: overrides.depsMaxBytes ?? bytesFromGb(DEFAULT_DEPS_MAX_GB),
    isolatedMaxAgeDays: overrides.isolatedMaxAgeDays ?? DEFAULT_ISOLATED_MAX_AGE_DAYS,
    isolatedMaxBytes: overrides.isolatedMaxBytes ?? bytesFromGb(DEFAULT_ISOLATED_MAX_GB),
    isolatedKeepLatest: overrides.isolatedKeepLatest ?? DEFAULT_ISOLATED_KEEP_LATEST,
  };
}

function decideIncrementalAction({ summary, now, thresholds }) {
  if (!summary.exists) {
    return { action: "missing", reasons: [] };
  }

  const reasons = [];
  const ageDays = (now.getTime() - summary.latestMtimeMs) / MS_PER_DAY;
  if (ageDays >= thresholds.incrementalMaxAgeDays) {
    reasons.push(`age ${ageDays.toFixed(1)}d >= ${thresholds.incrementalMaxAgeDays}d`);
  }
  if (summary.bytes >= thresholds.incrementalMaxBytes) {
    reasons.push(
      `size ${formatBytes(summary.bytes)} >= ${formatBytes(thresholds.incrementalMaxBytes)}`,
    );
  }

  if (reasons.length === 0) {
    return { action: "ok", reasons: [] };
  }

  return { action: "prune", reasons };
}

function decideDepsAction({ summary, thresholds }) {
  if (!summary.exists) {
    return { action: "missing", reasons: [] };
  }

  if (summary.bytes < thresholds.depsMaxBytes) {
    return { action: "ok", reasons: [] };
  }

  return {
    action: "blocked",
    reasons: [`size ${formatBytes(summary.bytes)} >= ${formatBytes(thresholds.depsMaxBytes)}`],
  };
}

async function removeDirectory(targetPath) {
  await rm(targetPath, { recursive: true, force: true });
}

async function inspectChildDirectories(targetPath) {
  try {
    const rootStats = await stat(targetPath);
    if (!rootStats.isDirectory()) {
      return { exists: false, bytes: 0, latestMtimeMs: 0, children: [] };
    }

    const entries = await readdir(targetPath, { withFileTypes: true });
    const children = [];
    let totalBytes = 0;
    let latestMtimeMs = rootStats.mtimeMs;

    for (const entry of entries) {
      if (!entry.isDirectory()) {
        continue;
      }

      const childPath = path.join(targetPath, entry.name);
      const summary = await inspectTree(childPath);
      totalBytes += summary.bytes;
      latestMtimeMs = Math.max(latestMtimeMs, summary.latestMtimeMs);
      children.push({
        name: entry.name,
        path: childPath,
        bytes: summary.bytes,
        latestMtimeMs: summary.latestMtimeMs,
      });
    }

    children.sort((a, b) => b.latestMtimeMs - a.latestMtimeMs || a.name.localeCompare(b.name));

    return { exists: true, bytes: totalBytes, latestMtimeMs, children };
  } catch (error) {
    if (error && typeof error === "object" && "code" in error && error.code === "ENOENT") {
      return { exists: false, bytes: 0, latestMtimeMs: 0, children: [] };
    }
    throw error;
  }
}

function decideIsolatedAction({ summary, now, thresholds, mode }) {
  if (!summary.exists) {
    return { action: "missing", reasons: [], prunePaths: [] };
  }

  if (mode === "clean") {
    return {
      action: summary.children.length > 0 ? "prune" : "ok",
      reasons: summary.children.length > 0 ? ["manual clean requested"] : [],
      prunePaths: summary.children.map((child) => child.path),
    };
  }

  const prunePaths = new Set();
  const reasons = [];

  for (const child of summary.children) {
    const ageDays = (now.getTime() - child.latestMtimeMs) / MS_PER_DAY;
    if (ageDays >= thresholds.isolatedMaxAgeDays) {
      prunePaths.add(child.path);
    }
  }
  if (prunePaths.size > 0) {
    reasons.push(`stale runs >= ${thresholds.isolatedMaxAgeDays}d`);
  }

  if (summary.children.length > thresholds.isolatedKeepLatest) {
    for (const child of summary.children.slice(thresholds.isolatedKeepLatest)) {
      prunePaths.add(child.path);
    }
    reasons.push(`keep latest ${thresholds.isolatedKeepLatest} isolated runs`);
  }

  let remainingBytes = summary.bytes;
  for (const child of summary.children) {
    if (prunePaths.has(child.path)) {
      remainingBytes -= child.bytes;
    }
  }
  if (remainingBytes > thresholds.isolatedMaxBytes) {
    for (const child of [...summary.children].reverse()) {
      if (!prunePaths.has(child.path)) {
        prunePaths.add(child.path);
        remainingBytes -= child.bytes;
      }
      if (remainingBytes <= thresholds.isolatedMaxBytes) {
        break;
      }
    }
    reasons.push(`total size > ${formatBytes(thresholds.isolatedMaxBytes)}`);
  }

  if (prunePaths.size === 0) {
    return { action: "ok", reasons: [], prunePaths: [] };
  }

  return { action: "prune", reasons, prunePaths: [...prunePaths] };
}

export async function setTreeMtime(targetPath, when) {
  const moment = when instanceof Date ? when : new Date(when);
  const stack = [targetPath];
  while (stack.length > 0) {
    const currentPath = stack.pop();
    const entries = await readdir(currentPath, { withFileTypes: true });
    for (const entry of entries) {
      const entryPath = path.join(currentPath, entry.name);
      if (entry.isDirectory()) {
        stack.push(entryPath);
      }
      await utimes(entryPath, moment, moment);
    }
    await utimes(currentPath, moment, moment);
  }
}

function describeIncrementalOutcome(action) {
  switch (action) {
    case "pruned":
      return "auto-pruned local incremental cache";
    case "would-prune":
      return "stale incremental cache detected (CI read-only)";
    case "ok":
      return "incremental cache within policy";
    case "missing":
      return "incremental cache not present";
    default:
      return action;
  }
}

function describeDepsOutcome(action) {
  switch (action) {
    case "blocked":
      return "deps cache exceeds policy and requires manual cleanup";
    case "ok":
      return "deps cache within policy";
    case "missing":
      return "deps cache not present";
    case "pruned":
      return "manual deps cleanup completed";
    default:
      return action;
  }
}

function describeIsolatedOutcome(action) {
  switch (action) {
    case "pruned":
      return "auto-pruned local isolated cargo runs";
    case "would-prune":
      return "stale isolated cargo runs detected (CI read-only)";
    case "ok":
      return "isolated cargo runs within policy";
    case "missing":
      return "isolated cargo runs not present";
    default:
      return action;
  }
}

export async function runBuildCacheCheck(options = {}) {
  const projectRoot = options.projectRoot ?? process.cwd();
  const debugRoot = path.join(projectRoot, DEFAULT_DEBUG_ROOT);
  const isolatedRoot = path.join(projectRoot, DEFAULT_ISOLATED_ROOT);
  const incrementalPath = path.join(debugRoot, "incremental");
  const depsPath = path.join(debugRoot, "deps");
  const now = options.now ?? new Date();
  const mode = options.mode ?? "check";
  const ci = options.ci ?? false;
  const includeDeps = options.includeDeps ?? false;
  const thresholds = buildThresholds(options.thresholds);

  const incrementalSummary = await inspectTree(incrementalPath);
  const depsSummary = await inspectTree(depsPath);
  const isolatedSummary = await inspectChildDirectories(isolatedRoot);

  const incrementalDecision =
    mode === "clean"
      ? { action: incrementalSummary.exists ? "prune" : "missing", reasons: [] }
      : decideIncrementalAction({ summary: incrementalSummary, now, thresholds });
  const depsDecision =
    mode === "clean" && includeDeps
      ? { action: depsSummary.exists ? "prune" : "missing", reasons: [] }
      : decideDepsAction({ summary: depsSummary, thresholds });
  const isolatedDecision = decideIsolatedAction({ summary: isolatedSummary, now, thresholds, mode });

  const incrementalResult = {
    path: incrementalPath,
    bytes: incrementalSummary.bytes,
    reasons: incrementalDecision.reasons,
    action: incrementalDecision.action,
    reclaimedBytes: 0,
  };
  const depsResult = {
    path: depsPath,
    bytes: depsSummary.bytes,
    reasons: depsDecision.reasons,
    action: depsDecision.action,
    reclaimedBytes: 0,
  };
  const isolatedResult = {
    path: isolatedRoot,
    bytes: isolatedSummary.bytes,
    reasons: isolatedDecision.reasons,
    action: isolatedDecision.action,
    prunedCount: 0,
    reclaimedBytes: 0,
  };

  if (incrementalDecision.action === "prune") {
    if (ci) {
      incrementalResult.action = "would-prune";
    } else {
      await removeDirectory(incrementalPath);
      incrementalResult.action = "pruned";
      incrementalResult.reclaimedBytes = incrementalSummary.bytes;
    }
  }

  if (depsDecision.action === "prune") {
    if (ci) {
      depsResult.action = "blocked";
      depsResult.reasons = ["CI clean mode does not delete deps caches."];
    } else {
      await removeDirectory(depsPath);
      depsResult.action = "pruned";
      depsResult.reclaimedBytes = depsSummary.bytes;
    }
  }

  if (isolatedDecision.action === "prune") {
    isolatedResult.prunedCount = isolatedDecision.prunePaths.length;
    if (ci) {
      isolatedResult.action = "would-prune";
    } else {
      let reclaimedBytes = 0;
      for (const targetPath of isolatedDecision.prunePaths) {
        const child = isolatedSummary.children.find((entry) => entry.path === targetPath);
        reclaimedBytes += child?.bytes ?? 0;
      }
      for (const targetPath of isolatedDecision.prunePaths) {
        await removeDirectory(targetPath);
      }
      isolatedResult.action = "pruned";
      isolatedResult.reclaimedBytes = reclaimedBytes;
    }
  }

  const exitCode = depsResult.action === "blocked" ? 1 : 0;

  return {
    debugRoot,
    isolatedRoot,
    thresholds,
    incremental: incrementalResult,
    deps: depsResult,
    isolated: isolatedResult,
    exitCode,
  };
}

function parseArgs(argv) {
  const options = {
    ci: false,
    includeDeps: false,
    mode: "check",
    thresholds: {},
    hook: null,
  };

  for (const arg of argv) {
    if (arg === "--ci") {
      options.ci = true;
      continue;
    }
    if (arg === "--include-deps") {
      options.includeDeps = true;
      continue;
    }
    if (arg.startsWith("--mode=")) {
      options.mode = arg.slice("--mode=".length);
      continue;
    }
    if (arg.startsWith("--hook=")) {
      options.hook = arg.slice("--hook=".length);
      continue;
    }
    if (arg.startsWith("--incremental-max-age-days=")) {
      options.thresholds.incrementalMaxAgeDays = Number.parseInt(
        arg.slice("--incremental-max-age-days=".length),
        10,
      );
      continue;
    }
    if (arg.startsWith("--incremental-max-gb=")) {
      const value = Number.parseInt(arg.slice("--incremental-max-gb=".length), 10);
      options.thresholds.incrementalMaxBytes = bytesFromGb(value);
      continue;
    }
    if (arg.startsWith("--deps-max-gb=")) {
      const value = Number.parseInt(arg.slice("--deps-max-gb=".length), 10);
      options.thresholds.depsMaxBytes = bytesFromGb(value);
      continue;
    }
    if (arg.startsWith("--isolated-max-age-days=")) {
      options.thresholds.isolatedMaxAgeDays = Number.parseInt(
        arg.slice("--isolated-max-age-days=".length),
        10,
      );
      continue;
    }
    if (arg.startsWith("--isolated-max-gb=")) {
      const value = Number.parseInt(arg.slice("--isolated-max-gb=".length), 10);
      options.thresholds.isolatedMaxBytes = bytesFromGb(value);
      continue;
    }
    if (arg.startsWith("--isolated-keep-latest=")) {
      options.thresholds.isolatedKeepLatest = Number.parseInt(
        arg.slice("--isolated-keep-latest=".length),
        10,
      );
    }
  }

  return options;
}

function printSummary(result, options) {
  const hookLabel = options.hook ? ` (${options.hook})` : "";
  console.log(`[build-cache] debug root${hookLabel}: ${result.debugRoot}`);
  console.log(
    `[build-cache] incremental: ${describeIncrementalOutcome(result.incremental.action)} (${formatBytes(result.incremental.bytes)})`,
  );
  if (result.incremental.reasons.length > 0) {
    for (const reason of result.incremental.reasons) {
      console.log(`[build-cache] incremental reason: ${reason}`);
    }
  }
  if (result.incremental.reclaimedBytes > 0) {
    console.log(
      `[build-cache] incremental reclaimed: ${formatBytes(result.incremental.reclaimedBytes)}`,
    );
  }

  console.log(
    `[build-cache] deps: ${describeDepsOutcome(result.deps.action)} (${formatBytes(result.deps.bytes)})`,
  );
  if (result.deps.reasons.length > 0) {
    for (const reason of result.deps.reasons) {
      console.log(`[build-cache] deps reason: ${reason}`);
    }
  }
  if (result.deps.reclaimedBytes > 0) {
    console.log(`[build-cache] deps reclaimed: ${formatBytes(result.deps.reclaimedBytes)}`);
  }

  console.log(
    `[build-cache] isolated: ${describeIsolatedOutcome(result.isolated.action)} (${formatBytes(result.isolated.bytes)})`,
  );
  if (result.isolated.reasons.length > 0) {
    for (const reason of result.isolated.reasons) {
      console.log(`[build-cache] isolated reason: ${reason}`);
    }
  }
  if (result.isolated.prunedCount > 0) {
    console.log(`[build-cache] isolated pruned directories: ${result.isolated.prunedCount}`);
  }
  if (result.isolated.reclaimedBytes > 0) {
    console.log(`[build-cache] isolated reclaimed: ${formatBytes(result.isolated.reclaimedBytes)}`);
  }

  if (result.deps.action === "blocked") {
    console.error(
      "[build-cache] Run `pnpm cache:build:clean -- --include-deps` after stopping local cargo/pnpm app processes.",
    );
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const result = await runBuildCacheCheck(options);
  printSummary(result, options);
  if (result.exitCode !== 0) {
    process.exit(result.exitCode);
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : null;
const currentPath = fileURLToPath(import.meta.url);

if (invokedPath && currentPath === invokedPath) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  });
}
