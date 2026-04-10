import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";

function runGit(args) {
  return spawnSync("git", args, {
    cwd: process.cwd(),
    stdio: "pipe",
    encoding: "utf8",
  });
}

function trimOutput(value) {
  return typeof value === "string" ? value.trim() : "";
}

function main() {
  const repoCheck = runGit(["rev-parse", "--is-inside-work-tree"]);
  if (repoCheck.status !== 0 || trimOutput(repoCheck.stdout) !== "true") {
    console.log("[git-hooks] Skipping hook install outside a git worktree.");
    return;
  }

  const desiredHooksPath = ".githooks";
  const currentHooksPath = runGit(["config", "--local", "--get", "core.hooksPath"]);
  if (trimOutput(currentHooksPath.stdout) === desiredHooksPath) {
    console.log("[git-hooks] core.hooksPath already points to .githooks.");
    return;
  }

  const setHooksPath = runGit(["config", "--local", "core.hooksPath", desiredHooksPath]);
  if (setHooksPath.status !== 0) {
    const stderr = trimOutput(setHooksPath.stderr) || "unknown git config failure";
    throw new Error(`[git-hooks] Failed to set core.hooksPath to ${desiredHooksPath}: ${stderr}`);
  }

  console.log(
    `[git-hooks] Installed repository hooks from ${path.join(process.cwd(), desiredHooksPath)}.`,
  );
}

main();
