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

if (import.meta.url === `file://${process.argv[1]?.replace(/\\/g, "/")}`) {
  const [, , chromeUserDataDir, command, extensionOrigin] = process.argv;
  if (!chromeUserDataDir || !command || !extensionOrigin) {
    console.error(
      "Usage: node scripts/install-chrome-native-host.mjs <chromeUserDataDir> <command> <extensionOrigin>",
    );
    process.exit(1);
  }

  const manifestPath = writeNativeHostManifest({
    chromeUserDataDir,
    hostName: "dev.workclaw.runtime",
    command,
    extensionOrigins: [extensionOrigin],
  });
  console.log(`Native host manifest written to ${manifestPath}`);
}
