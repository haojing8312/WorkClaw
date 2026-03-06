import test from "node:test";
import assert from "node:assert/strict";

import { buildDoctorReport, formatReport, resolvePnpmVersion } from "./doctor-windows.mjs";

test("healthy windows toolchain report has no blocking findings", () => {
  const report = buildDoctorReport({
    platform: "win32",
    nodeVersion: "v20.11.0",
    pnpmVersion: "9.0.0",
    rustcVersion: "rustc 1.86.0 (dummy)",
    rustupShow: "installed targets: x86_64-pc-windows-msvc",
    linkPaths: ["C:\\VS\\VC\\Tools\\MSVC\\14.40.0\\bin\\HostX64\\x64\\link.exe"],
    libEnv: "C:\\VS\\VC\\Tools\\MSVC\\14.40.0\\lib\\x64",
    visualStudio: { hasStable: true, hasInsiders: false },
    windowsSdk: { present: true },
    msvcTools: { present: true },
    errorText: "",
  });

  assert.equal(report.status, "pass");
  assert.equal(report.findings.filter((finding) => finding.severity === "fail").length, 0);
  assert.doesNotMatch(formatReport(report), /\[FAIL\]/);
});

test("missing link.exe is a blocking finding", () => {
  const report = buildDoctorReport({
    platform: "win32",
    nodeVersion: "v20.11.0",
    pnpmVersion: "9.0.0",
    rustcVersion: "rustc 1.86.0 (dummy)",
    rustupShow: "installed targets: x86_64-pc-windows-msvc",
    linkPaths: [],
    libEnv: "C:\\VS\\VC\\Tools\\MSVC\\14.40.0\\lib\\x64",
    visualStudio: { hasStable: true, hasInsiders: false },
    windowsSdk: { present: true },
    msvcTools: { present: true },
    errorText: "",
  });

  assert.equal(report.status, "fail");
  assert.ok(report.findings.some((finding) => finding.id === "link" && finding.severity === "fail"));
});

test("msvcrt linker error maps to MSVC workload guidance", () => {
  const report = buildDoctorReport({
    platform: "win32",
    nodeVersion: "v20.11.0",
    pnpmVersion: "9.0.0",
    rustcVersion: "rustc 1.86.0 (dummy)",
    rustupShow: "installed targets: x86_64-pc-windows-msvc",
    linkPaths: ["C:\\VS\\VC\\Tools\\MSVC\\14.40.0\\bin\\HostX64\\x64\\link.exe"],
    libEnv: "",
    visualStudio: { hasStable: false, hasInsiders: false },
    windowsSdk: { present: false },
    msvcTools: { present: false },
    errorText: "LINK : fatal error LNK1104: cannot open file 'msvcrt.lib'",
  });

  const errorFinding = report.findings.find((finding) => finding.id === "msvcrt");
  assert.ok(errorFinding, "Expected an msvcrt-specific finding");
  assert.equal(errorFinding.severity, "fail");
  assert.match(errorFinding.recommendation, /Desktop development with C\+\+|Windows SDK/i);
});

test("insiders-only visual studio install is a warning, not a blocking failure", () => {
  const report = buildDoctorReport({
    platform: "win32",
    nodeVersion: "v20.11.0",
    pnpmVersion: "9.0.0",
    rustcVersion: "rustc 1.86.0 (dummy)",
    rustupShow: "installed targets: x86_64-pc-windows-msvc",
    linkPaths: ["C:\\VS\\VC\\Tools\\MSVC\\14.40.0\\bin\\HostX64\\x64\\link.exe"],
    libEnv: "C:\\VS\\VC\\Tools\\MSVC\\14.40.0\\lib\\x64",
    visualStudio: { hasStable: false, hasInsiders: true },
    windowsSdk: { present: true },
    msvcTools: { present: true },
    errorText: "",
  });

  const vsFinding = report.findings.find((finding) => finding.id === "visual-studio");
  assert.ok(vsFinding, "Expected a Visual Studio baseline finding");
  assert.equal(vsFinding.severity, "warn");
  assert.equal(report.status, "warn");
});

test("pnpm version can be inferred from npm user agent when pnpm is not in PATH", () => {
  const pnpmVersion = resolvePnpmVersion({
    env: {
      npm_config_user_agent: "pnpm/9.15.4 npm/? node/v20.20.0 win32 x64",
    },
    run: () => ({ ok: false, stdout: "", stderr: "", status: 1 }),
  });

  assert.equal(pnpmVersion, "9.15.4");
});
