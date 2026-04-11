import path from "node:path";
import { pathToFileURL } from "node:url";
import { collectArtifactsSignals } from "./lib/repo-hygiene/collect-artifacts-signals.mjs";
import { collectDeadcodeSignals } from "./lib/repo-hygiene/collect-deadcode-signals.mjs";
import { collectDriftSignals } from "./lib/repo-hygiene/collect-drift-signals.mjs";
import { writeRepoHygieneReport } from "./lib/repo-hygiene/write-report.mjs";

const SUPPORTED_MODES = new Set(["all", "deadcode", "drift", "artifacts"]);

function parseArgs(argv) {
  const args = {
    outputDir: ".artifacts/repo-hygiene",
    mode: "all",
  };

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value.startsWith("--output-dir=")) {
      args.outputDir = value.slice("--output-dir=".length);
      continue;
    }
    if (value === "--output-dir") {
      args.outputDir = argv[index + 1];
      index += 1;
      continue;
    }

    if (value.startsWith("--mode=")) {
      args.mode = value.slice("--mode=".length);
      continue;
    }
    if (value === "--mode") {
      args.mode = argv[index + 1];
      index += 1;
    }
  }

  return args;
}

function buildCountsByCategory(findings) {
  return findings.reduce((counts, finding) => {
    const category = finding.category ?? "uncategorized";
    counts[category] = (counts[category] ?? 0) + 1;
    return counts;
  }, {});
}

function resolveCollectorPlan(mode, collectors) {
  if (!SUPPORTED_MODES.has(mode)) {
    throw new Error(`Unsupported repo hygiene mode: ${mode}`);
  }

  switch (mode) {
    case "deadcode":
      return {
        deadcode: collectors.deadcode,
        artifacts: [],
        drift: [],
      };
    case "drift":
      return {
        deadcode: [],
        artifacts: [],
        drift: collectors.drift,
      };
    case "artifacts":
      return {
        deadcode: [],
        artifacts: collectors.artifacts,
        drift: [],
      };
    default:
      return {
        deadcode: collectors.deadcode,
        artifacts: collectors.artifacts,
        drift: collectors.drift,
      };
  }
}

async function executeCollector(target, mode) {
  if (Array.isArray(target)) {
    return target;
  }

  return target({ mode });
}

export async function runRepoHygieneReview(options = {}) {
  const mode = options.mode ?? "all";
  const outputDir = path.resolve(options.outputDir ?? ".artifacts/repo-hygiene");
  const collectors = options.collectors ?? {
    deadcode: collectDeadcodeSignals,
    artifacts: collectArtifactsSignals,
    drift: collectDriftSignals,
  };
  const collectorPlan = resolveCollectorPlan(mode, collectors);

  const [deadcode, artifacts, drift] = await Promise.all([
    executeCollector(collectorPlan.deadcode, mode),
    executeCollector(collectorPlan.artifacts, mode),
    executeCollector(collectorPlan.drift, mode),
  ]);

  const findings = [...deadcode, ...artifacts, ...drift];
  const report = {
    generatedAt: new Date().toISOString(),
    mode,
    findings,
    countsByCategory: buildCountsByCategory(findings),
  };

  return writeRepoHygieneReport(outputDir, report);
}

async function main(argv = process.argv.slice(2)) {
  const args = parseArgs(argv);
  await runRepoHygieneReview(args);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  });
}

export {
  buildCountsByCategory,
  collectArtifactsSignals,
  collectDeadcodeSignals,
  collectDriftSignals,
  parseArgs,
  resolveCollectorPlan,
};
