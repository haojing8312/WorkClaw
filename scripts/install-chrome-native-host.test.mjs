import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";

import {
  buildNativeHostManifest,
  resolveNativeHostManifestPath,
} from "./install-chrome-native-host.mjs";

test("buildNativeHostManifest creates a Chrome native messaging manifest", () => {
  const manifest = buildNativeHostManifest({
    hostName: "dev.workclaw.runtime",
    command: "C:\\WorkClaw\\native-host.cmd",
    extensionOrigins: ["chrome-extension://abcdefghijklmnop/"],
  });

  assert.deepEqual(manifest, {
    name: "dev.workclaw.runtime",
    description: "WorkClaw browser bridge native host",
    path: "C:\\WorkClaw\\native-host.cmd",
    type: "stdio",
    allowed_origins: ["chrome-extension://abcdefghijklmnop/"],
  });
});

test("resolveNativeHostManifestPath uses the Chrome native messaging host directory", () => {
  const resolved = resolveNativeHostManifestPath(
    "C:\\Users\\tester\\AppData\\Local\\Google\\Chrome\\User Data",
    "dev.workclaw.runtime",
  );

  assert.equal(
    resolved,
    path.join(
      "C:\\Users\\tester\\AppData\\Local\\Google\\Chrome\\User Data",
      "NativeMessagingHosts",
      "dev.workclaw.runtime.json",
    ),
  );
});
