import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

export function buildNativeHostManifest({ hostName, command, extensionOrigins }) {
  return {
    name: hostName,
    description: "WorkClaw browser bridge native host",
    path: command,
    type: "stdio",
    allowed_origins: extensionOrigins,
  };
}

export function resolveNativeHostManifestPath(chromeUserDataDir, hostName) {
  return path.join(chromeUserDataDir, "NativeMessagingHosts", `${hostName}.json`);
}

export function buildWindowsNativeHostLauncher({ nodePath, scriptPath, baseUrl }) {
  return [
    "@echo off",
    `set "WORKCLAW_BROWSER_BRIDGE_BASE_URL=${baseUrl}"`,
    `"${nodePath}" "${scriptPath}"`,
    "",
  ].join("\r\n");
}

export function writeNativeHostManifest({ chromeUserDataDir, hostName, command, extensionOrigins }) {
  const manifestPath = resolveNativeHostManifestPath(chromeUserDataDir, hostName);
  mkdirSync(path.dirname(manifestPath), { recursive: true });
  writeFileSync(
    manifestPath,
    JSON.stringify(buildNativeHostManifest({ hostName, command, extensionOrigins }), null, 2),
    "utf8",
  );
  return manifestPath;
}

export function writeWindowsNativeHostInstallation({
  chromeUserDataDir,
  hostName,
  extensionOrigin,
  launcherPath,
  nodePath,
  scriptPath,
  baseUrl,
}) {
  mkdirSync(path.dirname(launcherPath), { recursive: true });
  writeFileSync(
    launcherPath,
    buildWindowsNativeHostLauncher({ nodePath, scriptPath, baseUrl }),
    "utf8",
  );

  const manifestPath = writeNativeHostManifest({
    chromeUserDataDir,
    hostName,
    command: launcherPath,
    extensionOrigins: [extensionOrigin],
  });

  return {
    launcherPath,
    manifestPath,
  };
}

if (import.meta.url === `file://${process.argv[1]?.replace(/\\/g, "/")}`) {
  const [, , chromeUserDataDir, commandOrLauncherPath, extensionOrigin, nodePath, scriptPath, baseUrl] = process.argv;
  if (!chromeUserDataDir || !commandOrLauncherPath || !extensionOrigin) {
    console.error(
      "Usage: node scripts/install-chrome-native-host.mjs <chromeUserDataDir> <commandOrLauncherPath> <extensionOrigin> [nodePath scriptPath baseUrl]",
    );
    process.exit(1);
  }

  if (nodePath && scriptPath) {
    const result = writeWindowsNativeHostInstallation({
      chromeUserDataDir,
      hostName: "dev.workclaw.runtime",
      extensionOrigin,
      launcherPath: commandOrLauncherPath,
      nodePath,
      scriptPath,
      baseUrl: baseUrl || "http://127.0.0.1:4312",
    });
    console.log(`Native host launcher written to ${result.launcherPath}`);
    console.log(`Native host manifest written to ${result.manifestPath}`);
  } else {
    const manifestPath = writeNativeHostManifest({
      chromeUserDataDir,
      hostName: "dev.workclaw.runtime",
      command: commandOrLauncherPath,
      extensionOrigins: [extensionOrigin],
    });
    console.log(`Native host manifest written to ${manifestPath}`);
  }
}
