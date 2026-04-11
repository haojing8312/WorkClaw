import path from "node:path";
import { readdir } from "node:fs/promises";

const SUPPORTED_MODES = new Set(["all", "artifacts"]);

const ROOT_TEMP_PATTERNS = [
  {
    pattern: /^tmp[-._]/i,
    reason: "tmp-prefixed root file",
  },
  {
    pattern: /^temp[-._]/i,
    reason: "temp-prefixed root file",
  },
  {
    pattern: /^\.tmp/i,
    reason: "dot-tmp root file",
  },
  {
    pattern: /\.log$/i,
    reason: "root log file",
  },
  {
    pattern: /\.tgz$/i,
    reason: "root archive artifact",
  },
];

export async function collectArtifactsSignals(options = {}) {
  const mode = options.mode ?? "all";
  if (!SUPPORTED_MODES.has(mode)) {
    return [];
  }

  const rootDir = path.resolve(options.rootDir ?? process.cwd());
  const entries = await readdir(rootDir, { withFileTypes: true }).catch(() => []);

  return entries
    .filter((entry) => entry.isFile())
    .map((entry) => {
      const match = ROOT_TEMP_PATTERNS.find(({ pattern }) => pattern.test(entry.name));
      if (!match) {
        return null;
      }

      return {
        category: "temporary-artifacts",
        confidence: "likely",
        action: "review-first",
        source: entry.name,
        detail: match.reason,
      };
    })
    .filter(Boolean)
    .sort((left, right) => left.source.localeCompare(right.source));
}
