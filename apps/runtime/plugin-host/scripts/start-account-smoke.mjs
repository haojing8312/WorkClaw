import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const workspaceRuntimeDir = path.resolve(process.cwd(), "..");
const pluginHostDir = path.resolve(workspaceRuntimeDir, "plugin-host");
const shimPluginSdkRoot = path.join(pluginHostDir, "openclaw", "plugin-sdk");

function parseArgs(argv) {
  const args = {
    pluginRoot: "",
    fixtureName: "plugin-start-account-smoke",
    fixtureRoot: "",
    accountId: "default",
    configJson: "",
    configFile: "",
    durationMs: 2000,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--plugin-root") {
      args.pluginRoot = argv[index + 1] ?? "";
      index += 1;
      continue;
    }
    if (arg === "--fixture-name") {
      args.fixtureName = argv[index + 1] ?? args.fixtureName;
      index += 1;
      continue;
    }
    if (arg === "--fixture-root") {
      args.fixtureRoot = argv[index + 1] ?? "";
      index += 1;
      continue;
    }
    if (arg === "--account-id") {
      args.accountId = argv[index + 1] ?? args.accountId;
      index += 1;
      continue;
    }
    if (arg === "--config-json") {
      args.configJson = argv[index + 1] ?? "";
      index += 1;
      continue;
    }
    if (arg === "--config-file") {
      args.configFile = argv[index + 1] ?? "";
      index += 1;
      continue;
    }
    if (arg === "--duration-ms") {
      args.durationMs = Number(argv[index + 1] ?? args.durationMs) || args.durationMs;
      index += 1;
    }
  }

  if (!args.pluginRoot.trim()) {
    throw new Error("--plugin-root is required");
  }

  return args;
}

function resolveFixtureWorkspaceRoot(fixtureRoot) {
  const explicitRoot = typeof fixtureRoot === "string" && fixtureRoot.trim() ? fixtureRoot.trim() : "";
  if (explicitRoot) {
    return path.resolve(explicitRoot);
  }
  return path.join(workspaceRuntimeDir, ".workclaw-plugin-host-fixtures");
}

function normalizeRegistrationMode(value) {
  return value === "setup-only" || value === "setup-runtime" ? value : "full";
}

function resolvePluginManifest(pluginRoot) {
  const packageJsonPath = path.join(pluginRoot, "package.json");
  const packageJson = fs.existsSync(packageJsonPath)
    ? JSON.parse(fs.readFileSync(packageJsonPath, "utf8"))
    : {};
  const packageManifest = packageJson.openclaw ?? {};
  const manifestPath = path.join(pluginRoot, "openclaw.plugin.json");
  if (fs.existsSync(manifestPath)) {
    return {
      ...packageManifest,
      ...JSON.parse(fs.readFileSync(manifestPath, "utf8")),
      extensions: Array.isArray(packageManifest.extensions) ? packageManifest.extensions : undefined,
      install: packageManifest.install,
      channel: packageManifest.channel,
    };
  }
  return packageManifest;
}

function resolvePluginEntry(pluginRoot, manifest) {
  const registrationMode = normalizeRegistrationMode(manifest.registrationMode);
  const extensions = Array.isArray(manifest.extensions) ? manifest.extensions : [];
  const setupEntry =
    typeof manifest.setupEntry === "string" && manifest.setupEntry.trim() ? manifest.setupEntry : undefined;

  const relativeEntry =
    registrationMode === "setup-only"
      ? setupEntry
      : registrationMode === "setup-runtime"
        ? setupEntry ?? extensions[0]
        : extensions[0];

  if (!relativeEntry || !relativeEntry.trim()) {
    throw new Error("plugin manifest does not provide an entrypoint");
  }

  return path.resolve(pluginRoot, relativeEntry);
}

function createPluginRegistry() {
  return {
    channels: [],
    tools: [],
    cliEntries: [],
    commands: [],
    gatewayMethods: {},
    hooks: {
      before_tool_call: [],
      after_tool_call: [],
    },
  };
}

function createPluginRuntime(config) {
  const records = [];

  function createLogger(scope) {
    return {
      debug: (...args) => records.push({ level: "debug", scope, args }),
      info: (...args) => records.push({ level: "info", scope, args }),
      warn: (...args) => records.push({ level: "warn", scope, args }),
      error: (...args) => records.push({ level: "error", scope, args }),
    };
  }

  return {
    config: {
      loadConfig: async () => config,
    },
    channel: {
      text: {
        chunkMarkdownText(text, limit) {
          if (limit <= 0) {
            return [text];
          }
          const chunks = [];
          for (let index = 0; index < text.length; index += limit) {
            chunks.push(text.slice(index, index + limit));
          }
          return chunks;
        },
        convertMarkdownTables(text) {
          return text;
        },
      },
    },
    logging: {
      records,
      getChildLogger({ scope }) {
        return createLogger(scope);
      },
    },
    log(...args) {
      records.push({ level: "info", scope: "runtime", args });
    },
    error(...args) {
      records.push({ level: "error", scope: "runtime", args });
    },
  };
}

function createPluginApi(registry, { runtime, logger, config, registrationMode }) {
  return {
    runtime,
    logger,
    config,
    registrationMode,
    registerChannel(input) {
      registry.channels.push(input.plugin);
    },
    registerTool(tool) {
      registry.tools.push(tool);
    },
    registerCli(cliEntry, registration) {
      if (typeof cliEntry === "function") {
        cliEntry({
          program: {
            commands: [],
            command() {
              const chain = {
                description() {
                  return chain;
                },
                option() {
                  return chain;
                },
                action() {
                  return chain;
                },
              };
              return chain;
            },
          },
          config,
          logger,
        });
      }
      registry.cliEntries.push({ entry: cliEntry, registration });
    },
    registerGatewayMethod(name, handler) {
      registry.gatewayMethods[name] = handler;
    },
    registerCommand(command) {
      registry.commands.push(command);
    },
    on(eventName, handler) {
      registry.hooks[eventName].push(handler);
    },
  };
}

function rewriteImportsInFixture(rootDir) {
  const stack = [rootDir];

  while (stack.length > 0) {
    const currentDir = stack.pop();
    if (!currentDir) {
      continue;
    }

    for (const entry of fs.readdirSync(currentDir, { withFileTypes: true })) {
      const entryPath = path.join(currentDir, entry.name);
      if (entry.isDirectory()) {
        stack.push(entryPath);
        continue;
      }

      if (!entry.isFile() || !entry.name.endsWith(".js")) {
        continue;
      }

      const relativeShimRoot = path.relative(path.dirname(entryPath), shimPluginSdkRoot).replace(/\\/g, "/");
      const normalizeRelativeImport = (rawSpecifier) => {
        const resolvedPath = path.resolve(path.dirname(entryPath), rawSpecifier);
        const fileCandidate = `${resolvedPath}.js`;
        const indexCandidate = path.join(resolvedPath, "index.js");
        let normalizedTarget = rawSpecifier;

        if (!path.extname(rawSpecifier)) {
          if (fs.existsSync(fileCandidate)) {
            normalizedTarget = `${rawSpecifier}.js`;
          } else if (fs.existsSync(indexCandidate)) {
            normalizedTarget = `${rawSpecifier}/index.js`;
          }
        }

        return normalizedTarget.replace(/\\/g, "/");
      };

      const rewritten = fs
        .readFileSync(entryPath, "utf8")
        .replaceAll(/(['"])openclaw\/plugin-sdk\/compat\1/g, (_match, quote) => `${quote}${relativeShimRoot}/compat.js${quote}`)
        .replaceAll(/(['"])openclaw\/plugin-sdk\/feishu\1/g, (_match, quote) => `${quote}${relativeShimRoot}/feishu.js${quote}`)
        .replaceAll(/(['"])openclaw\/plugin-sdk\1/g, (_match, quote) => `${quote}${relativeShimRoot}/index.js${quote}`)
        .replaceAll(/from\s+(['"])(\.\.?\/[^'"]+)\1/g, (_match, quote, specifier) => `from ${quote}${normalizeRelativeImport(specifier)}${quote}`)
        .replaceAll(/import\s+(['"])(\.\.?\/[^'"]+)\1/g, (_match, quote, specifier) => `import ${quote}${normalizeRelativeImport(specifier)}${quote}`);
      fs.writeFileSync(entryPath, rewritten, "utf8");
    }
  }
}

function findNearestNodeModulesDir(startPath) {
  let current = path.resolve(startPath);
  while (true) {
    if (path.basename(current) === "node_modules") {
      return current;
    }
    const parent = path.dirname(current);
    if (parent === current) {
      return null;
    }
    current = parent;
  }
}

function prepareFixture(pluginRoot, fixtureWorkspaceRoot, fixtureName) {
  const targetRoot = path.join(fixtureWorkspaceRoot, fixtureName);
  fs.rmSync(targetRoot, { recursive: true, force: true });
  fs.mkdirSync(path.dirname(targetRoot), { recursive: true });
  fs.cpSync(pluginRoot, targetRoot, { recursive: true });
  const sourceNodeModules = findNearestNodeModulesDir(pluginRoot);
  const targetNodeModules = path.join(targetRoot, "node_modules");
  if (sourceNodeModules && !fs.existsSync(targetNodeModules)) {
    fs.mkdirSync(path.dirname(targetNodeModules), { recursive: true });
    fs.symlinkSync(sourceNodeModules, targetNodeModules, process.platform === "win32" ? "junction" : "dir");
  }
  rewriteImportsInFixture(targetRoot);
  return targetRoot;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const fixtureWorkspaceRoot = resolveFixtureWorkspaceRoot(args.fixtureRoot);
  const pluginRoot = path.resolve(args.pluginRoot);
  const preparedRoot = prepareFixture(pluginRoot, fixtureWorkspaceRoot, args.fixtureName);
  const manifest = resolvePluginManifest(preparedRoot);
  const entryPath = resolvePluginEntry(preparedRoot, manifest);
  const registry = createPluginRegistry();
  const configSource = args.configFile.trim() ? fs.readFileSync(path.resolve(args.configFile), "utf8") : args.configJson;
  const config = configSource.trim() ? JSON.parse(configSource) : {};
  const runtime = createPluginRuntime(config);
  const logger = runtime.logging.getChildLogger({ scope: "plugin-host-start-account-smoke" });
  const api = createPluginApi(registry, {
    runtime,
    logger,
    config,
    registrationMode: normalizeRegistrationMode(manifest.registrationMode),
  });

  const loadedModule = await import(pathToFileURL(entryPath).href);
  const plugin = loadedModule.default ?? loadedModule;
  if (typeof plugin.register !== "function") {
    throw new Error("plugin module must export a register(api) function");
  }
  await plugin.register(api);

  const channel = registry.channels.find((entry) => entry?.id === "feishu");
  if (!channel?.gateway?.startAccount) {
    throw new Error("feishu gateway.startAccount not found");
  }
  if (typeof channel?.config?.resolveAccount !== "function") {
    throw new Error("feishu channel config.resolveAccount not found");
  }

  const account = channel.config.resolveAccount(config, args.accountId);
  const runtimeStatus = {};
  const abortController = new AbortController();
  setTimeout(() => abortController.abort(), args.durationMs);

  try {
    await channel.gateway.startAccount({
      cfg: config,
      accountId: args.accountId,
      account,
      runtime,
      abortSignal: abortController.signal,
      log: logger,
      getStatus: () => runtimeStatus,
      setStatus: (next) => Object.assign(runtimeStatus, next),
      channelRuntime: runtime.channel,
    });
    process.stdout.write(
      `${JSON.stringify(
        {
          ok: true,
          pluginRoot,
          preparedRoot,
          entryPath,
          runtimeStatus,
          logRecordCount: runtime.logging.records.length,
          logs: runtime.logging.records,
        },
        null,
        2,
      )}\n`,
    );
  } catch (error) {
    process.stdout.write(
      `${JSON.stringify(
        {
          ok: false,
          pluginRoot,
          preparedRoot,
          entryPath,
          runtimeStatus,
          error: String(error),
          stack: error instanceof Error ? error.stack : null,
          logRecordCount: runtime.logging.records.length,
          logs: runtime.logging.records,
        },
        null,
        2,
      )}\n`,
    );
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error("[start-account-smoke] failed");
  console.error(error);
  process.exitCode = 1;
});
