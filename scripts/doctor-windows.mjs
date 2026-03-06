import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SEVERITY_ORDER = {
  pass: 0,
  warn: 1,
  fail: 2,
};

function makeFinding(id, severity, summary, recommendation) {
  return { id, severity, summary, recommendation };
}

function normalizeLines(text) {
  return (text || "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function detectSeverity(findings) {
  if (findings.some((finding) => finding.severity === "fail")) {
    return "fail";
  }

  if (findings.some((finding) => finding.severity === "warn")) {
    return "warn";
  }

  return "pass";
}

function parseArgs(args) {
  const parsed = { errorFile: "" };

  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === "--error-file" && args[index + 1]) {
      parsed.errorFile = args[index + 1];
      index += 1;
    }
  }

  return parsed;
}

function runCommand(command, args) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    windowsHide: true,
  });

  return {
    ok: result.status === 0,
    status: result.status ?? 1,
    stdout: result.stdout || "",
    stderr: result.stderr || "",
  };
}

export function resolvePnpmVersion({ env = process.env, run = runCommand } = {}) {
  const userAgent = env.npm_config_user_agent || "";
  const match = userAgent.match(/pnpm\/([^\s]+)/i);
  if (match) {
    return match[1];
  }

  const execPath = env.npm_execpath || "";
  if (execPath.toLowerCase().includes("pnpm")) {
    const versionResult = run(process.execPath, [execPath, "--version"]);
    if (versionResult.ok) {
      return versionResult.stdout.trim();
    }
  }

  const pnpmResult = run("pnpm", ["-v"]);
  return pnpmResult.ok ? pnpmResult.stdout.trim() : "";
}

function detectVisualStudio(env) {
  const programFiles = [env.ProgramFiles, env["ProgramFiles(x86)"]].filter(Boolean);
  const stablePaths = [];
  const insidersPaths = [];

  for (const base of programFiles) {
    for (const edition of ["BuildTools", "Community", "Professional", "Enterprise"]) {
      stablePaths.push(path.join(base, "Microsoft Visual Studio", "2022", edition));
    }

    for (const previewRoot of [
      path.join(base, "Microsoft Visual Studio", "2022", "Preview"),
      path.join(base, "Microsoft Visual Studio", "18", "Preview"),
      path.join(base, "Microsoft Visual Studio", "18", "Insiders"),
    ]) {
      insidersPaths.push(previewRoot);
    }
  }

  return {
    hasStable: stablePaths.some((candidate) => existsSync(candidate)),
    hasInsiders: insidersPaths.some((candidate) => existsSync(candidate)),
    stablePaths,
    insidersPaths,
  };
}

function detectWindowsSdk(env) {
  const candidates = [
    env.WindowsSdkDir,
    env["WindowsSdkDir_10"],
    env.ProgramFiles && path.join(env.ProgramFiles, "Windows Kits", "10"),
    env["ProgramFiles(x86)"] && path.join(env["ProgramFiles(x86)"], "Windows Kits", "10"),
  ].filter(Boolean);

  return {
    present: candidates.some((candidate) => existsSync(candidate)),
    candidates,
  };
}

function detectMsvcTools(env) {
  const candidates = [];

  for (const base of [env.ProgramFiles, env["ProgramFiles(x86)"]].filter(Boolean)) {
    for (const release of ["2022", "18"]) {
      for (const edition of ["BuildTools", "Community", "Professional", "Enterprise", "Preview", "Insiders"]) {
        candidates.push(
          path.join(base, "Microsoft Visual Studio", release, edition, "VC", "Tools", "MSVC"),
        );
      }
    }
  }

  return {
    present: candidates.some((candidate) => existsSync(candidate)),
    candidates,
  };
}

function analyzeErrorText(errorText) {
  if (/LNK1104/i.test(errorText) && /msvcrt\.lib/i.test(errorText)) {
    return makeFinding(
      "msvcrt",
      "fail",
      "The linker cannot open msvcrt.lib, which points to a broken or incomplete MSVC toolchain.",
      "Install stable Visual Studio 2022 Build Tools with Desktop development with C++ and the Windows SDK, then reopen the terminal and rerun the build.",
    );
  }

  return null;
}

export function buildDoctorReport(input) {
  const findings = [];
  const hasWindowsTarget =
    /x86_64-pc-windows-msvc/i.test(input.rustupShow || "") ||
    /host:\s*x86_64-pc-windows-msvc/i.test(input.rustcVersion || "");

  if (input.platform !== "win32") {
    findings.push(
      makeFinding(
        "platform",
        "warn",
        `This doctor command is Windows-first; current platform is ${input.platform || "unknown"}.`,
        "Use it primarily for Windows contributor setup. Non-Windows support can be added later.",
      ),
    );
  }

  findings.push(
    input.nodeVersion
      ? makeFinding("node", "pass", `Node.js detected: ${input.nodeVersion}.`, "No action required.")
      : makeFinding("node", "fail", "Node.js is not available in PATH.", "Install Node.js 20+ and retry."),
  );

  findings.push(
    input.pnpmVersion
      ? makeFinding("pnpm", "pass", `pnpm detected: ${input.pnpmVersion}.`, "No action required.")
      : makeFinding("pnpm", "fail", "pnpm is not available in PATH.", "Install pnpm and retry."),
  );

  findings.push(
    input.rustcVersion
      ? makeFinding("rustc", "pass", "Rust toolchain is available.", "No action required.")
      : makeFinding("rustc", "fail", "rustc is not available in PATH.", "Install Rust stable and retry."),
  );

  findings.push(
    hasWindowsTarget
      ? makeFinding(
          "target",
          "pass",
          "Rust Windows MSVC target is present.",
          "No action required.",
        )
      : makeFinding(
          "target",
          "fail",
          "Rust Windows MSVC target is missing.",
          "Run `rustup target add x86_64-pc-windows-msvc` and retry.",
        ),
  );

  findings.push(
    (input.linkPaths || []).length > 0
      ? makeFinding("link", "pass", "link.exe is discoverable.", "No action required.")
      : makeFinding(
          "link",
          "fail",
          "link.exe is not discoverable from the current shell.",
          "Install Visual Studio 2022 Build Tools with Desktop development with C++ and open a fresh terminal.",
        ),
  );

  if (input.visualStudio?.hasStable) {
    findings.push(
      makeFinding(
        "visual-studio",
        "pass",
        "Stable Visual Studio Build Tools installation detected.",
        "No action required.",
      ),
    );
  } else if (input.visualStudio?.hasInsiders) {
    findings.push(
      makeFinding(
        "visual-studio",
        "warn",
        "Only Visual Studio Preview/Insiders installation detected.",
        "Stable Visual Studio 2022 Build Tools is the supported baseline. Preview/Insiders is best effort only.",
      ),
    );
  } else {
    findings.push(
      makeFinding(
        "visual-studio",
        "fail",
        "No supported Visual Studio Build Tools installation was detected.",
        "Install stable Visual Studio 2022 Build Tools with Desktop development with C++.",
      ),
    );
  }

  findings.push(
    input.windowsSdk?.present
      ? makeFinding("windows-sdk", "pass", "Windows SDK detected.", "No action required.")
      : makeFinding(
          "windows-sdk",
          "fail",
          "Windows SDK was not detected.",
          "Install the Windows 10/11 SDK from the Visual Studio installer and retry.",
        ),
  );

  findings.push(
    input.msvcTools?.present
      ? makeFinding("msvc-tools", "pass", "MSVC tool directories detected.", "No action required.")
      : makeFinding(
          "msvc-tools",
          "fail",
          "MSVC tool directories were not detected.",
          "Install Desktop development with C++ in stable Visual Studio 2022 Build Tools.",
        ),
  );

  if (!input.libEnv) {
    findings.push(
      makeFinding(
        "lib-env",
        "warn",
        "LIB environment variable is empty in this shell.",
        "If builds fail during linking, reopen the terminal after installing Build Tools or use a Developer Command Prompt.",
      ),
    );
  } else {
    findings.push(
      makeFinding("lib-env", "pass", "LIB environment variable is populated.", "No action required."),
    );
  }

  const errorFinding = analyzeErrorText(input.errorText || "");
  if (errorFinding) {
    findings.push(errorFinding);
  }

  return {
    status: detectSeverity(findings),
    findings,
    environment: {
      linkPaths: input.linkPaths || [],
      visualStudio: input.visualStudio,
      windowsSdk: input.windowsSdk,
      msvcTools: input.msvcTools,
    },
  };
}

export function formatReport(report) {
  const summary = [
    "WorkClaw Windows doctor",
    `Overall status: ${report.status.toUpperCase()}`,
    "",
  ];

  const findingLines = [...report.findings]
    .sort((left, right) => SEVERITY_ORDER[right.severity] - SEVERITY_ORDER[left.severity])
    .flatMap((finding) => [
      `[${finding.severity.toUpperCase()}] ${finding.summary}`,
      `  ${finding.recommendation}`,
    ]);

  return [...summary, ...findingLines].join("\n");
}

export function collectDoctorInput({ env = process.env, args = process.argv.slice(2) } = {}) {
  const parsedArgs = parseArgs(args);
  const rustcResult = runCommand("rustc", ["-vV"]);
  const rustupResult = runCommand("rustup", ["show"]);
  const linkResult = runCommand("where", ["link"]);

  let errorText = "";
  if (parsedArgs.errorFile && existsSync(parsedArgs.errorFile)) {
    errorText = readFileSync(parsedArgs.errorFile, "utf8");
  }

  return {
    platform: process.platform,
    nodeVersion: process.version,
    pnpmVersion: resolvePnpmVersion({ env }),
    rustcVersion: rustcResult.ok ? rustcResult.stdout.trim() : "",
    rustupShow: rustupResult.ok ? rustupResult.stdout.trim() : "",
    linkPaths: linkResult.ok ? normalizeLines(linkResult.stdout) : [],
    libEnv: env.LIB || "",
    visualStudio: detectVisualStudio(env),
    windowsSdk: detectWindowsSdk(env),
    msvcTools: detectMsvcTools(env),
    errorText,
  };
}

const isDirectRun = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];

if (isDirectRun) {
  const report = buildDoctorReport(collectDoctorInput());
  console.log(formatReport(report));
  process.exitCode = report.status === "fail" ? 1 : 0;
}
