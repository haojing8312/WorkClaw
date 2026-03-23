import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const workspaceRuntimeDir = path.resolve(process.cwd(), "..");
const pluginHostDir = path.resolve(workspaceRuntimeDir, "plugin-host");
const shimPluginSdkRoot = path.join(pluginHostDir, "openclaw", "plugin-sdk");

function parseArgs(argv) {
  const args = {
    pluginRoot: "",
    fixtureName: "plugin-inspect",
    fixtureRoot: "",
    channelSnapshot: "",
    configJson: "",
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
    if (arg === "--channel-snapshot") {
      args.channelSnapshot = argv[index + 1] ?? "";
      index += 1;
      continue;
    }
    if (arg === "--config-json") {
      args.configJson = argv[index + 1] ?? "";
      index += 1;
    }
  }

  if (!args.pluginRoot.trim()) {
    throw new Error("--plugin-root is required");
  }

  return args;
}

function resolveFixtureWorkspaceRoot(fixtureRoot) {
  const explicitRoot = readString(fixtureRoot);
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
      extensions: Array.isArray(packageManifest.extensions)
        ? packageManifest.extensions
        : undefined,
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

function asRecord(value) {
  return value != null && typeof value === "object" ? value : undefined;
}

function readString(value) {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function readStringArray(value) {
  return Array.isArray(value)
    ? value.filter((item) => typeof item === "string" && item.trim().length > 0)
    : [];
}

function summarizeRegistry(registry) {
  return {
    channels: registry.channels.map((channel) => {
      const record = asRecord(channel) ?? {};
      const meta = asRecord(record.meta);
      const reload = asRecord(record.reload);
      const messaging = asRecord(record.messaging);
      const targetResolver = asRecord(messaging?.targetResolver);
      return {
        id: readString(record.id),
        meta: meta
          ? {
              id: readString(meta.id),
              label: readString(meta.label),
              selectionLabel: readString(meta.selectionLabel),
              docsPath: readString(meta.docsPath),
              docsLabel: readString(meta.docsLabel),
              blurb: readString(meta.blurb),
              aliases: readStringArray(meta.aliases),
              order: typeof meta.order === "number" ? meta.order : undefined,
            }
          : undefined,
        capabilities: asRecord(record.capabilities),
        reloadConfigPrefixes: readStringArray(reload?.configPrefixes),
        hasPairing: Boolean(record.pairing),
        hasSetup: Boolean(record.setup),
        hasOnboarding: Boolean(record.onboarding),
        hasDirectory: Boolean(record.directory),
        hasOutbound: Boolean(record.outbound),
        hasThreading: Boolean(record.threading),
        hasActions: Boolean(record.actions),
        hasStatus: Boolean(record.status),
        targetHint: readString(targetResolver?.hint),
      };
    }),
    tools: registry.tools.map((tool) => {
      const record = asRecord(tool) ?? {};
      return {
        id: readString(record.id),
        name: readString(record.name),
        title: readString(record.title),
        description: readString(record.description),
      };
    }),
    commandNames: registry.commands
      .map((command) => {
        const record = asRecord(command);
        return readString(record?.name) ?? readString(record?.id) ?? readString(record?.command);
      })
      .filter(Boolean),
    cliCommandNames: registry.cliEntries
      .map((entry) => readStringArray(asRecord(asRecord(entry)?.registration)?.commands))
      .flat(),
    gatewayMethods: Object.keys(registry.gatewayMethods),
    hookCounts: Object.fromEntries(
      Object.entries(registry.hooks).map(([eventName, handlers]) => [eventName, handlers.length]),
    ),
  };
}

function buildChannelSnapshot(channel, config) {
  const channelId = readString(channel?.id) ?? readString(channel?.meta?.id) ?? "unknown";
  const accountIds =
    typeof channel?.config?.listAccountIds === "function" ? channel.config.listAccountIds(config) : [];
  const defaultAccountId =
    typeof channel?.config?.defaultAccountId === "function"
      ? channel.config.defaultAccountId(config)
      : undefined;
  const accounts = Array.isArray(accountIds)
    ? accountIds.map((accountId) => {
        const resolvedAccount =
          typeof channel?.config?.resolveAccount === "function"
            ? channel.config.resolveAccount(config, accountId)
            : undefined;
        const describedAccount =
          typeof channel?.config?.describeAccount === "function" && resolvedAccount
            ? channel.config.describeAccount(resolvedAccount)
            : undefined;
        const allowFrom =
          typeof channel?.config?.resolveAllowFrom === "function"
            ? channel.config.resolveAllowFrom({ cfg: config, accountId })
            : [];
        const warnings =
          typeof channel?.security?.collectWarnings === "function"
            ? channel.security.collectWarnings({ cfg: config, accountId })
            : [];

        return {
          accountId,
          account: resolvedAccount ?? null,
          describedAccount: describedAccount ?? null,
          allowFrom: Array.isArray(allowFrom) ? allowFrom : [],
          warnings: Array.isArray(warnings) ? warnings : [],
        };
      })
    : [];

  return {
    channelId,
    defaultAccountId,
    accountIds: Array.isArray(accountIds) ? accountIds : [],
    accounts,
    reloadConfigPrefixes: readStringArray(channel?.reload?.configPrefixes),
    targetHint: readString(channel?.messaging?.targetResolver?.hint),
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
        .replaceAll(
          /(['"])openclaw\/plugin-sdk\/compat\1/g,
          (_match, quote) => `${quote}${relativeShimRoot}/compat.js${quote}`,
        )
        .replaceAll(
          /(['"])openclaw\/plugin-sdk\/feishu\1/g,
          (_match, quote) => `${quote}${relativeShimRoot}/feishu.js${quote}`,
        )
        .replaceAll(
          /(['"])openclaw\/plugin-sdk\1/g,
          (_match, quote) => `${quote}${relativeShimRoot}/index.js${quote}`,
        )
        .replaceAll(
          /from\s+(['"])(\.\.?\/[^'"]+)\1/g,
          (_match, quote, specifier) => `from ${quote}${normalizeRelativeImport(specifier)}${quote}`,
        )
        .replaceAll(
          /import\s+(['"])(\.\.?\/[^'"]+)\1/g,
          (_match, quote, specifier) => `import ${quote}${normalizeRelativeImport(specifier)}${quote}`,
        );
      fs.writeFileSync(entryPath, rewritten, "utf8");
    }
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
    fs.symlinkSync(
      sourceNodeModules,
      targetNodeModules,
      process.platform === "win32" ? "junction" : "dir",
    );
  }
  rewriteImportsInFixture(targetRoot);
  return targetRoot;
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

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const fixtureWorkspaceRoot = resolveFixtureWorkspaceRoot(args.fixtureRoot);
  const pluginRoot = path.resolve(args.pluginRoot);
  const preparedRoot = prepareFixture(pluginRoot, fixtureWorkspaceRoot, args.fixtureName);
  const manifest = resolvePluginManifest(preparedRoot);
  const entryPath = resolvePluginEntry(preparedRoot, manifest);
  const registry = createPluginRegistry();
  const config = args.configJson.trim()
    ? JSON.parse(args.configJson)
    : {
        channels: {
          feishu: {
            enabled: true,
            accounts: {
              default: {
                appId: "demo-app",
                appSecret: "demo-secret",
                enabled: true,
              },
            },
          },
        },
        tools: {
          profile: "default",
        },
        plugins: {
          entries: {},
        },
      };
  const runtime = createPluginRuntime(config);
  const logger = runtime.logging.getChildLogger({ scope: "plugin-host-inspect" });
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

  if (args.channelSnapshot.trim()) {
    const requestedChannel = args.channelSnapshot.trim().toLowerCase();
    const channel = registry.channels.find((entry) => {
      const record = asRecord(entry);
      const id = readString(record?.id) ?? readString(asRecord(record?.meta)?.id);
      return id?.toLowerCase() === requestedChannel;
    });
    if (!channel) {
      throw new Error(`channel not found in plugin registry: ${requestedChannel}`);
    }

    process.stdout.write(
      `${JSON.stringify(
        {
          pluginRoot,
          preparedRoot,
          manifest,
          entryPath,
          snapshot: buildChannelSnapshot(channel, config),
          logRecordCount: runtime.logging.records.length,
        },
        null,
        2,
      )}\n`,
    );
    return;
  }

  process.stdout.write(
    `${JSON.stringify(
      {
        pluginRoot,
        preparedRoot,
        manifest,
        entryPath,
        summary: summarizeRegistry(registry),
        logRecordCount: runtime.logging.records.length,
      },
      null,
      2,
    )}\n`,
  );
}

main().catch((error) => {
  console.error("[inspect-plugin] failed");
  console.error(error);
  process.exitCode = 1;
});
