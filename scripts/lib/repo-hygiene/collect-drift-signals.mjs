import path from "node:path";
import { access, readFile } from "node:fs/promises";

const SUPPORTED_MODES = new Set(["all", "drift"]);

async function readJson(filePath) {
  const raw = await readFile(filePath, "utf8").catch(() => null);
  if (!raw) {
    return null;
  }

  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function buildFinding(detail, source) {
  return {
    category: "stale-doc-or-skill-reference",
    confidence: "probable",
    action: "review-first",
    source,
    detail,
  };
}

async function pathExists(filePath) {
  try {
    await access(filePath);
    return true;
  } catch {
    return false;
  }
}

export async function collectDriftSignals(options = {}) {
  const mode = options.mode ?? "all";
  if (!SUPPORTED_MODES.has(mode)) {
    return [];
  }

  const rootDir = path.resolve(options.rootDir ?? process.cwd());
  const packageJsonPath = path.join(rootDir, "package.json");
  const repoHygieneDocPath = path.join(rootDir, "docs", "maintenance", "repo-hygiene.md");

  const [packageJson, repoHygieneDoc] = await Promise.all([
    readJson(packageJsonPath),
    readFile(repoHygieneDocPath, "utf8").catch(() => ""),
  ]);

  const findings = [];
  const reviewScript = packageJson?.scripts?.["review:repo-hygiene"];

  if (!reviewScript) {
    findings.push(
      buildFinding("Missing review:repo-hygiene package script", "package.json"),
    );
  } else {
    const expectedTarget = "scripts/review-repo-hygiene.mjs";
    if (!reviewScript.includes(expectedTarget)) {
      findings.push(
        buildFinding(
          "Unexpected review:repo-hygiene script target",
          "package.json",
        ),
      );
    } else if (!(await pathExists(path.join(rootDir, expectedTarget)))) {
      findings.push(
        buildFinding(
          "Missing review:repo-hygiene script target",
          expectedTarget,
        ),
      );
    }
  }

  if (!repoHygieneDoc.includes("pnpm review:repo-hygiene")) {
    findings.push(
      buildFinding(
        "Missing pnpm review:repo-hygiene maintenance doc reference",
        "docs/maintenance/repo-hygiene.md",
      ),
    );
  }

  if (!repoHygieneDoc.includes("workclaw-repo-hygiene-review")) {
    findings.push(
      buildFinding(
        "Missing workclaw-repo-hygiene-review maintenance doc reference",
        "docs/maintenance/repo-hygiene.md",
      ),
    );
  }

  return findings;
}
