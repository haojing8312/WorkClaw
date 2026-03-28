import { spawnSync } from "node:child_process";
import { copyFileSync, cpSync, existsSync, lstatSync, readdirSync, readlinkSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const scriptPath = fileURLToPath(import.meta.url);
const projectRoot = path.resolve(scriptDir, "..");
const localStoreDir = path.join(projectRoot, ".pnpm-store-local");
const bundleDir = path.join(
  projectRoot,
  "apps",
  "runtime",
  "src-tauri",
  "resources",
  "sidecar-runtime",
);
const bundledNodeName = process.platform === "win32" ? "node.exe" : "node";
const mcpSdkExampleSuffixes = [
  path.join("node_modules", "@modelcontextprotocol", "sdk", "dist", "cjs", "examples"),
  path.join("node_modules", "@modelcontextprotocol", "sdk", "dist", "esm", "examples"),
];

function resolvePnpmRunner() {
  return {
    command: process.platform === "win32" ? "pnpm.cmd" : "pnpm",
    args: [],
  };
}

export function buildDeployCommand(runner, pnpmMajor, targetDir, baseEnv = process.env) {
  const deployArgs = [
    ...runner.args,
    "--filter",
    "workclaw-runtime-sidecar",
    "deploy",
    "--prod",
    "--config.bin-links=false",
    "--store-dir",
    localStoreDir,
  ];
  if (pnpmMajor >= 10) {
    deployArgs.push("--legacy");
  }
  deployArgs.push(targetDir);

  return {
    command: runner.command,
    args: deployArgs,
    env: {
      ...baseEnv,
      npm_config_bin_links: "false",
      pnpm_config_bin_links: "false",
      npm_config_store_dir: localStoreDir,
      pnpm_config_store_dir: localStoreDir,
      NPM_CONFIG_BIN_LINKS: "false",
      PNPM_CONFIG_BIN_LINKS: "false",
      NPM_CONFIG_STORE_DIR: localStoreDir,
      PNPM_CONFIG_STORE_DIR: localStoreDir,
    },
  };
}

export function isRetryableWindowsDeployError(output, platform = process.platform) {
  if (platform !== "win32") {
    return false;
  }

  const text = String(output ?? "");
  return (
    text.includes("playwright.CMD") ||
    text.includes("playwright.ps1") ||
    text.includes("Failed to create bin at") ||
    text.includes("EPERM") ||
    text.includes("ENOENT: no such file or directory, chmod") ||
    (text.includes("ERR_PNPM_UNKNOWN") && text.includes("unknown error, stat"))
  );
}

function listPackageRoots(baseDir, { treatScopedPackages = false } = {}) {
  if (!existsSync(baseDir)) {
    return [];
  }

  const packageRoots = [];
  for (const entry of readdirSync(baseDir, { withFileTypes: true })) {
    if (!entry.isDirectory() && !entry.isSymbolicLink()) {
      continue;
    }

    const entryPath = path.join(baseDir, entry.name);
    if (!treatScopedPackages || !entry.name.startsWith("@")) {
      packageRoots.push(entryPath);
      continue;
    }

    for (const scopedEntry of readdirSync(entryPath, { withFileTypes: true })) {
      if (!scopedEntry.isDirectory() && !scopedEntry.isSymbolicLink()) {
        continue;
      }
      packageRoots.push(path.join(entryPath, scopedEntry.name));
    }
  }

  return packageRoots;
}

export function listNonRuntimeBundlePaths(targetDir) {
  const candidatePaths = new Set();
  const candidateRoots = [
    targetDir,
    ...listPackageRoots(path.join(targetDir, "node_modules", ".pnpm")),
    ...listPackageRoots(path.join(targetDir, "node_modules", ".pnpm", "node_modules"), {
      treatScopedPackages: true,
    }),
  ];
  for (const candidateRoot of candidateRoots) {
    for (const suffix of mcpSdkExampleSuffixes) {
      const candidatePath = path.join(candidateRoot, suffix);
      if (existsSync(candidatePath)) {
        candidatePaths.add(candidatePath);
      }
    }
  }

  return [...candidatePaths].sort();
}

export function pruneNonRuntimeBundlePaths(targetDir) {
  const prunedPaths = listNonRuntimeBundlePaths(targetDir);
  for (const prunedPath of prunedPaths) {
    rmSync(prunedPath, { recursive: true, force: true });
  }
  return prunedPaths;
}

function readPnpmMajorVersion(runner) {
  const result = spawnSync(runner.command, [...runner.args, "--version"], {
    cwd: projectRoot,
    encoding: "utf8",
    windowsHide: true,
    env: process.env,
    shell: process.platform === "win32",
  });

  if (result.status !== 0) {
    throw new Error(`Unable to detect pnpm version via ${runner.command} --version`);
  }

  const versionText = String(result.stdout ?? "").trim();
  const major = Number.parseInt(versionText.split(".")[0] ?? "", 10);
  if (!Number.isFinite(major)) {
    throw new Error(`Unexpected pnpm version output: ${versionText}`);
  }
  return major;
}

function runOrThrow(command, args, env = process.env) {
  const result = spawnSync(command, args, {
    cwd: projectRoot,
    stdio: "pipe",
    encoding: "utf8",
    windowsHide: true,
    env,
    shell: process.platform === "win32",
  });

  if (result.stdout) {
    process.stdout.write(result.stdout);
  }
  if (result.stderr) {
    process.stderr.write(result.stderr);
  }

  if (result.status !== 0) {
    const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
    const error = new Error(`Command failed: ${command} ${args.join(" ")}`);
    error.cause = { output, status: result.status };
    throw error;
  }
}

function removeDirForWindowsBuild(targetDir) {
  if (process.platform === "win32") {
    spawnSync("cmd.exe", ["/c", "rmdir", "/s", "/q", targetDir], {
      cwd: projectRoot,
      windowsHide: true,
      env: process.env,
    });
  }
  rmSync(targetDir, { recursive: true, force: true });
}

function listBrokenSymlinks(dir, broken = []) {
  if (!existsSync(dir)) {
    return broken;
  }

  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const entryPath = path.join(dir, entry.name);
    const stat = lstatSync(entryPath);
    if (stat.isSymbolicLink()) {
      try {
        // `realpathSync.native` would also work, but `cpSync` only needs to know whether
        // the current link resolves, so `existsSync` on the link target is enough here.
        const targetPath = readlinkSync(entryPath);
        const absoluteTarget = path.isAbsolute(targetPath)
          ? targetPath
          : path.resolve(path.dirname(entryPath), targetPath);
        if (!existsSync(absoluteTarget)) {
          broken.push({ linkPath: entryPath, targetPath: absoluteTarget });
        }
      } catch {
        broken.push({ linkPath: entryPath, targetPath: null });
      }
      continue;
    }

    if (entry.isDirectory()) {
      listBrokenSymlinks(entryPath, broken);
    }
  }

  return broken;
}

function resolveWorkspaceVirtualStoreTarget(targetDir, brokenTargetPath, workspaceRoot = projectRoot) {
  if (!brokenTargetPath) {
    return null;
  }

  const targetVirtualStore = path.join(targetDir, "node_modules", ".pnpm");
  const relativeTarget = path.relative(targetVirtualStore, brokenTargetPath);
  if (
    !relativeTarget ||
    relativeTarget.startsWith("..") ||
    path.isAbsolute(relativeTarget)
  ) {
    return null;
  }

  return path.join(workspaceRoot, "node_modules", ".pnpm", relativeTarget);
}

export function repairBrokenBundleLinks(targetDir, workspaceRoot = projectRoot) {
  const bundleVirtualStore = path.join(targetDir, "node_modules", ".pnpm");
  const repaired = [];

  for (const brokenLink of listBrokenSymlinks(bundleVirtualStore)) {
    const sourcePath = resolveWorkspaceVirtualStoreTarget(
      targetDir,
      brokenLink.targetPath,
      workspaceRoot,
    );
    if (!sourcePath || !existsSync(sourcePath)) {
      continue;
    }

    rmSync(brokenLink.linkPath, { recursive: true, force: true });
    cpSync(sourcePath, brokenLink.linkPath, { recursive: true, dereference: true });
    repaired.push({
      linkPath: brokenLink.linkPath,
      sourcePath,
    });
  }

  return repaired;
}

export function hasRequiredBundleOutputs(targetDir) {
  return (
    existsSync(path.join(targetDir, "package.json")) &&
    existsSync(path.join(targetDir, "dist", "index.js")) &&
    existsSync(path.join(targetDir, "node_modules"))
  );
}

function main() {
  const runner = resolvePnpmRunner();
  const pnpmMajor = readPnpmMajorVersion(runner);
  const deployCommand = buildDeployCommand(runner, pnpmMajor, bundleDir);
  let deployAttempt = 0;
  while (true) {
    deployAttempt += 1;
    removeDirForWindowsBuild(bundleDir);
    try {
      runOrThrow(deployCommand.command, deployCommand.args, deployCommand.env);
      break;
    } catch (error) {
      const output = error && typeof error === "object" && "cause" in error
        ? error.cause?.output
        : "";
      if (
        isRetryableWindowsDeployError(output) &&
        hasRequiredBundleOutputs(bundleDir)
      ) {
        console.warn("Continuing after Windows deploy warning because the sidecar bundle outputs are present.");
        break;
      }
      if (deployAttempt >= 2 || !isRetryableWindowsDeployError(output)) {
        throw error;
      }
      console.warn("Retrying sidecar runtime deploy after transient Windows deploy failure...");
    }
  }

  const distEntry = path.join(bundleDir, "dist", "index.js");
  if (!existsSync(distEntry)) {
    throw new Error(
      `Bundled sidecar runtime is missing ${distEntry}. Run the sidecar build before staging resources.`,
    );
  }

  for (const prunedPath of pruneNonRuntimeBundlePaths(bundleDir)) {
    console.log(`Pruned non-runtime sidecar bundle path: ${path.relative(projectRoot, prunedPath)}`);
  }

  for (const repairedLink of repairBrokenBundleLinks(bundleDir)) {
    console.log(
      `Repaired sidecar bundle link: ${path.relative(projectRoot, repairedLink.linkPath)} <= ${path.relative(projectRoot, repairedLink.sourcePath)}`,
    );
  }

  copyFileSync(process.execPath, path.join(bundleDir, bundledNodeName));
}

const isMainModule =
  typeof process.argv[1] === "string" &&
  path.resolve(process.argv[1]) === path.resolve(scriptPath);

if (isMainModule) {
  main();
}
