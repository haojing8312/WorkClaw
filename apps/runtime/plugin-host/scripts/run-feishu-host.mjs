import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";
import { pathToFileURL } from "node:url";

const workspaceRuntimeDir = path.resolve(process.cwd(), "..");
const pluginHostDir = path.resolve(workspaceRuntimeDir, "plugin-host");
const shimPluginSdkRoot = path.join(pluginHostDir, "openclaw", "plugin-sdk");
const shimPluginSdkCjsRoot = path.join(pluginHostDir, "openclaw", "plugin-sdk-cjs");

function emit(event, payload = {}) {
  process.stdout.write(`${JSON.stringify({ event, ...payload })}\n`);
}

function emitReplyLifecycle(lifecycle, phase, extra = {}) {
  emit("reply_lifecycle", {
    logicalReplyId: lifecycle.logicalReplyId,
    phase,
    channel: lifecycle.channel,
    ...(lifecycle.accountId ? { accountId: lifecycle.accountId } : {}),
    ...(lifecycle.threadId ? { threadId: lifecycle.threadId } : {}),
    ...(lifecycle.chatId ? { chatId: lifecycle.chatId } : {}),
    ...(lifecycle.messageId ? { messageId: lifecycle.messageId } : {}),
    ...extra,
  });
}

function stripPrefixedTarget(rawTarget, prefix) {
  const value = readString(rawTarget);
  if (!value) {
    return undefined;
  }
  if (value.startsWith(prefix)) {
    return value.slice(prefix.length).trim() || undefined;
  }
  return undefined;
}

function deriveDispatchThreadId(ctx) {
  const chatType = readString(ctx.ChatType) ?? "direct";
  const groupSubject = readString(ctx.GroupSubject);
  if (groupSubject) {
    return groupSubject;
  }

  const toChatId = stripPrefixedTarget(ctx.To, "chat:");
  if (toChatId) {
    return toChatId;
  }

  const explicitChatId = readString(ctx.ChatId);
  if (explicitChatId) {
    return explicitChatId;
  }

  const toUserId = stripPrefixedTarget(ctx.To, "user:");
  if (toUserId) {
    return toUserId;
  }

  const fromOpenId = stripPrefixedTarget(ctx.From, "feishu:");
  if (fromOpenId) {
    return fromOpenId;
  }

  if (chatType === "group") {
    return readString(ctx.ChatId) ?? "unknown-group";
  }

  return readString(ctx.SenderId) ?? "unknown-user";
}

function emitDispatchRequest(params) {
  const ctx = asRecord(params?.ctx) ?? {};
  const rawBody =
    readString(ctx.RawBody) ??
    readString(ctx.CommandBody) ??
    readString(ctx.BodyForAgent) ??
    readString(ctx.Body) ??
    "";
  const senderId = readString(ctx.SenderId) ?? "";
  const chatType = readString(ctx.ChatType) ?? "direct";
  const threadId = deriveDispatchThreadId(ctx);
  const chatId =
    stripPrefixedTarget(ctx.To, "chat:") ??
    readString(ctx.ChatId) ??
    undefined;
  const accountId = readString(ctx.AccountId) ?? "default";
  const messageId = readString(ctx.MessageSid);
  const roleId = readString(ctx.RoleId) ?? readString(ctx.TargetRoleId);

  emit("dispatch_request", {
    accountId,
    threadId,
    ...(chatId ? { chatId } : {}),
    senderId,
    messageId,
    text: rawBody,
    chatType,
    ...(roleId ? { roleId } : {}),
  });
}

function stringifyLogArgs(args) {
  return args
    .map((value) => {
      if (typeof value === "string") {
        return value;
      }
      try {
        return JSON.stringify(value);
      } catch {
        return String(value);
      }
    })
    .join(" ")
    .trim();
}

function installConsoleEventBridge() {
  const levels = new Map([
    ["debug", "debug"],
    ["info", "info"],
    ["log", "info"],
    ["warn", "warn"],
    ["error", "error"],
  ]);

  for (const [methodName, level] of levels.entries()) {
    console[methodName] = (...args) => {
      const message = stringifyLogArgs(args);
      if (!message) {
        return;
      }
      emit("log", {
        level,
        scope: "console",
        message,
      });
    };
  }
}

function parseArgs(argv) {
  const args = {
    pluginRoot: "",
    fixtureName: "plugin-feishu-runtime",
    fixtureRoot: "",
    accountId: "default",
    configJson: "",
    configFile: "",
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

function resolvePluginExport(loadedModule) {
  const candidates = [loadedModule, loadedModule?.default, loadedModule?.default?.default];
  for (const candidate of candidates) {
    if (candidate && typeof candidate.register === "function") {
      return candidate;
    }
  }
  return loadedModule?.default ?? loadedModule;
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

function asRecord(value) {
  return value != null && typeof value === "object" ? value : undefined;
}

function readString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function normalizeOutboundTarget(rawTarget) {
  const value = readString(rawTarget);
  if (!value) {
    return undefined;
  }
  if (value.startsWith("chat:")) {
    return value.slice("chat:".length).trim() || undefined;
  }
  if (value.startsWith("user:")) {
    return value.slice("user:".length).trim() || undefined;
  }
  if (value.startsWith("feishu:")) {
    return value.slice("feishu:".length).trim() || undefined;
  }
  return value;
}

function normalizeCommandPayload(payload) {
  if (!asRecord(payload)) {
    return undefined;
  }
  const requestId = readString(payload.requestId) ?? readString(payload.id) ?? readString(payload.request_id);
  const command = readString(payload.command);
  return {
    requestId,
    command,
    accountId: readString(payload.accountId) ?? readString(payload.account_id),
    target: readString(payload.target),
    threadId: readString(payload.threadId) ?? readString(payload.thread_id),
    messageId: readString(payload.messageId) ?? readString(payload.message_id),
    logicalReplyId: readString(payload.logicalReplyId) ?? readString(payload.logical_reply_id),
    finalState: readString(payload.finalState) ?? readString(payload.final_state),
    phase: readString(payload.phase),
    text: readString(payload.text),
    mode: readString(payload.mode),
  };
}

function normalizeAllowFrom(value) {
  if (Array.isArray(value)) {
    return value.map((entry) => String(entry ?? "").trim()).filter(Boolean);
  }
  const scalar = readString(value);
  return scalar ? [scalar] : [];
}

function resolveFeishuConfig(config) {
  return asRecord(asRecord(asRecord(config.channels)?.feishu) ?? {}) ?? {};
}

function resolveFeishuGroupConfig(config, groupId) {
  const feishu = resolveFeishuConfig(config);
  const groups = asRecord(feishu.groups);
  if (!groups) {
    return undefined;
  }
  const key = readString(groupId);
  if (key && asRecord(groups[key])) {
    return asRecord(groups[key]);
  }
  return asRecord(groups["*"]);
}

function resolveRequireMention(config, groupId) {
  const groupConfig = resolveFeishuGroupConfig(config, groupId);
  if (typeof groupConfig?.requireMention === "boolean") {
    return groupConfig.requireMention;
  }
  return resolveFeishuConfig(config).requireMention === true;
}

function resolveGroupPolicy(config, groupId) {
  const groupConfig = resolveFeishuGroupConfig(config, groupId);
  return readString(groupConfig?.groupPolicy) ?? readString(resolveFeishuConfig(config).groupPolicy) ?? "allowlist";
}

function resolveEffectiveAllowFrom(config, accountId) {
  const feishu = resolveFeishuConfig(config);
  const accounts = asRecord(feishu.accounts);
  const accountKey = readString(accountId) ?? "default";
  const account = asRecord(accounts?.[accountKey]) ?? {};
  const merged = [...normalizeAllowFrom(feishu.allowFrom), ...normalizeAllowFrom(account.allowFrom)];
  return Array.from(new Set(merged));
}

function resolveTypingIndicatorEnabled(config, accountId) {
  const feishu = resolveFeishuConfig(config);
  const accounts = asRecord(feishu.accounts);
  const accountKey = readString(accountId) ?? "default";
  const account = asRecord(accounts?.[accountKey]) ?? {};
  if (typeof account.typingIndicator === "boolean") {
    return account.typingIndicator;
  }
  if (typeof feishu.typingIndicator === "boolean") {
    return feishu.typingIndicator;
  }
  return true;
}

function createSessionKey(channel, peerKind, peerId, agentId) {
  const normalizedPeer = peerId.trim().toLowerCase() || "unknown";
  return `agent:${agentId}:${channel}:${peerKind}:${normalizedPeer}`;
}

function resolveAgentRoute(params) {
  const channel = readString(params.channel) ?? "unknown";
  const peerKind = readString(params.peer?.kind) ?? "direct";
  const peerId = readString(params.peer?.id) ?? "unknown";
  const agentId = "main";
  return {
    agentId,
    sessionKey: createSessionKey(channel, peerKind, peerId, agentId),
    matchedBy: "plugin-host-default",
  };
}

function createReplyDispatcher() {
  let sendChain = Promise.resolve();
  let waitEmitted = false;
  let idleReachedEmitted = false;
  let completeEmitted = false;
  const queuedCounts = {
    tool: 0,
    block: 0,
    final: 0,
  };

  const enqueue = (kind) => {
    queuedCounts[kind] += 1;
    sendChain = sendChain.then(async () => undefined);
    lifecycle?.record(
      kind === "tool"
        ? "tool_chunk_queued"
        : kind === "block"
          ? "block_chunk_queued"
          : "final_chunk_queued",
      { queuedCounts: { ...queuedCounts } },
    );
    return true;
  };
  let lifecycle = null;

  return {
    sendToolResult() {
      return enqueue("tool");
    },
    sendBlockReply() {
      return enqueue("block");
    },
    sendFinalReply() {
      return enqueue("final");
    },
    async waitForIdle() {
      if (!waitEmitted) {
        waitEmitted = true;
        lifecycle?.record("wait_for_idle", { queuedCounts: { ...queuedCounts } });
      }
      await sendChain;
      if (!idleReachedEmitted) {
        idleReachedEmitted = true;
        lifecycle?.record("idle_reached", { queuedCounts: { ...queuedCounts } });
      }
    },
    getQueuedCounts() {
      return { ...queuedCounts };
    },
    markComplete() {
      if (!completeEmitted) {
        completeEmitted = true;
        lifecycle?.record("fully_complete", { queuedCounts: { ...queuedCounts } });
      }
    },
    setLifecycle(lifecycleMeta) {
      lifecycle = lifecycleMeta;
    },
    getLifecycle() {
      return lifecycle ? { ...lifecycle } : undefined;
    },
  };
}

function generatePairingCode() {
  return Math.random().toString(36).slice(2, 10).toUpperCase();
}

function createPluginRuntime(config, options = {}) {
  const records = [];
  const systemRecords = [];
  const pairingRequests = new Map();
  let outboundSendSequence = 0;
  const processingReactions = asRecord(options.processingReactions) ?? {
    async start() {
      return false;
    },
    async stop() {
      return false;
    },
  };

  function pushRecord(level, scope, args) {
    const message = stringifyLogArgs(args);
    records.push({ level, scope, args, message });
    if (message) {
      emit("log", { level, scope, message });
    }
  }

  function createLogger(scope) {
    return {
      debug: (...args) => pushRecord("debug", scope, args),
      info: (...args) => pushRecord("info", scope, args),
      warn: (...args) => pushRecord("warn", scope, args),
      error: (...args) => pushRecord("error", scope, args),
    };
  }

  return {
    config: {
      loadConfig: () => config,
    },
    channel: {
      text: {
        chunkMarkdownText(text, limit) {
          if (limit <= 0) return [text];
          const chunks = [];
          for (let index = 0; index < text.length; index += limit) {
            chunks.push(text.slice(index, index + limit));
          }
          return chunks;
        },
        chunkTextWithMode(text, limit) {
          if (limit <= 0) return [text];
          const chunks = [];
          for (let index = 0; index < text.length; index += limit) {
            chunks.push(text.slice(index, index + limit));
          }
          return chunks;
        },
        resolveTextChunkLimit(_cfg, fallbackLimit) {
          return Math.max(1, fallbackLimit ?? 2000);
        },
        resolveChunkMode(_cfg, fallbackMode) {
          return fallbackMode ?? "newline";
        },
        resolveMarkdownTableMode(_cfg, fallbackMode) {
          return fallbackMode ?? "bullets";
        },
        convertMarkdownTables(text) {
          return text;
        },
      },
      groups: {
        resolveGroupPolicy(params) {
          return resolveGroupPolicy(params.cfg ?? config, params.groupId);
        },
        resolveRequireMention(params) {
          return resolveRequireMention(params.cfg ?? config, params.groupId);
        },
      },
      pairing: {
        async readAllowFromStore(params) {
          return resolveEffectiveAllowFrom(params?.cfg ?? config, params?.accountId);
        },
        async upsertPairingRequest(params) {
          const id = readString(params.id) ?? "pairing-request";
          const existing = pairingRequests.get(id);
          if (existing) {
            emit("pairing_request", {
              channel: existing.channel,
              accountId: existing.accountId,
              senderId: existing.id,
              code: existing.code,
              created: false,
            });
            return existing;
          }
          const record = {
            channel: readString(params.channel) ?? "feishu",
            accountId: readString(params.accountId) ?? "default",
            id,
            code: generatePairingCode(),
            meta: params.meta ?? {},
          };
          pairingRequests.set(id, record);
          emit("pairing_request", {
            channel: record.channel,
            accountId: record.accountId,
            senderId: record.id,
            code: record.code,
            created: true,
          });
          return record;
        },
        buildPairingReply(params) {
          const channel = readString(params.channel) ?? "feishu";
          const idLine = readString(params.idLine) ?? "Unknown sender";
          const code = readString(params.code) ?? "PAIRING";
          return [
            "OpenClaw: access not configured.",
            "",
            idLine,
            "",
            `Pairing code: ${code}`,
            "",
            "Ask the bot owner to approve with:",
            `openclaw pairing approve ${channel} ${code}`,
          ].join("\n");
        },
      },
      commands: {
        shouldComputeCommandAuthorized(rawBody) {
          return String(rawBody ?? "").trim().startsWith("/");
        },
        resolveCommandAuthorizedFromAuthorizers(params) {
          const authorizers = params.authorizers ?? [];
          const configured = authorizers.some((entry) => entry.configured);
          if (!configured) return true;
          return authorizers.some((entry) => entry.allowed);
        },
        isControlCommandMessage(rawBody) {
          return String(rawBody ?? "").trim().startsWith("/");
        },
      },
      routing: {
        resolveAgentRoute,
      },
      outbound: {
        async sendMessage(params) {
          const accountId = readString(params?.accountId) ?? "default";
          const target = normalizeOutboundTarget(params?.target);
          if (!target) {
            throw new Error("outbound target is required");
          }
          const text = readString(params?.text) ?? "";
          const mode = readString(params?.mode) ?? "text";
          const threadId = readString(params?.threadId);
          const sequence = ++outboundSendSequence;
          const result = {
            delivered: true,
            channel: "feishu",
            accountId,
            target,
            ...(threadId ? { threadId } : {}),
            text,
            mode,
            messageId: `om_outbound_${sequence}`,
            chatId: target,
            sequence,
          };

          systemRecords.push({
            message: "outbound-send-request",
            meta: {
              accountId,
              target,
              ...(threadId ? { threadId } : {}),
              text,
              mode,
              sequence,
            },
          });
          systemRecords.push({
            message: "outbound-send-result",
            meta: result,
          });

          return result;
        },
      },
      reply: {
        resolveEnvelopeFormatOptions() {
          return {};
        },
        formatAgentEnvelope(params) {
          return String(params.bodyForAgent ?? params.body ?? "");
        },
        finalizeInboundContext(ctx) {
          return { ...ctx };
        },
        createReplyDispatcherWithTyping() {
          const lifecycle = {
            logicalReplyId: `reply_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`,
            channel: "feishu",
          };
          const dispatcher = createReplyDispatcher();
          dispatcher.setLifecycle(lifecycle);
          return {
            dispatcher,
            replyOptions: {},
            markDispatchIdle() {
              return;
            },
            markRunComplete() {
              dispatcher.markComplete();
            },
          };
        },
        resolveHumanDelayConfig() {
          return { enabled: false, delayMs: 0 };
        },
        async withReplyDispatcher(params) {
          try {
            return await params.run();
          } finally {
            try {
              await params.dispatcher.waitForIdle();
            } finally {
              params.dispatcher.markComplete();
              const lifecycle = params.dispatcher.getLifecycle?.() ?? {
                logicalReplyId: `reply_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`,
                channel: "feishu",
              };
              emitReplyLifecycle(lifecycle, "dispatch_idle", {
                queuedCounts: params.dispatcher.getQueuedCounts?.() ?? { tool: 0, block: 0, final: 0 },
              });
              await params.onSettled?.();
            }
          }
        },
        async dispatchReplyFromConfig(params) {
          const dispatcher = params?.dispatcher;
          const ctx = asRecord(params?.ctx) ?? {};
          const chatId =
            stripPrefixedTarget(ctx.To, "chat:") ??
            readString(ctx.ChatId) ??
            undefined;
          const threadId = deriveDispatchThreadId(ctx);
          const accountId = readString(ctx.AccountId) ?? "default";
          const messageId = readString(ctx.MessageSid);
          const lifecycle = dispatcher?.getLifecycle?.() ?? {
            logicalReplyId: `reply_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`,
            channel: "feishu",
          };
          dispatcher?.setLifecycle?.({
            ...lifecycle,
            accountId,
            threadId,
            ...(chatId ? { chatId } : {}),
            ...(messageId ? { messageId } : {}),
            record(phase, extra = {}) {
              emitReplyLifecycle(
                {
                  logicalReplyId: lifecycle.logicalReplyId,
                  channel: "feishu",
                  accountId,
                  threadId,
                  ...(chatId ? { chatId } : {}),
                  ...(messageId ? { messageId } : {}),
                },
                phase,
                extra,
              );
            },
          });
          await processingReactions.start({
            messageId,
            accountId,
          });
          emitDispatchRequest(params);
          emitReplyLifecycle(
            {
              logicalReplyId: lifecycle.logicalReplyId,
              channel: "feishu",
              accountId,
              threadId,
              ...(chatId ? { chatId } : {}),
              ...(messageId ? { messageId } : {}),
            },
            "reply_started",
            { queuedCounts: dispatcher?.getQueuedCounts?.() ?? { tool: 0, block: 0, final: 0 } },
          );
          emitReplyLifecycle(
            {
              logicalReplyId: lifecycle.logicalReplyId,
              channel: "feishu",
              accountId,
              threadId,
              ...(chatId ? { chatId } : {}),
              ...(messageId ? { messageId } : {}),
            },
            "processing_started",
          );
          return { queuedFinal: false, counts: { final: 0 } };
        },
        async dispatchReplyWithBufferedBlockDispatcher() {
          return { delivered: false };
        },
      },
      media: {
        async saveMediaBuffer(params) {
          return readString(params.fileName) ?? `media-${Date.now()}`;
        },
      },
    },
    system: {
      records: systemRecords,
      enqueueSystemEvent(message, meta) {
        systemRecords.push({ message, meta });
      },
    },
    logging: {
      records,
      getChildLogger({ scope }) {
        return createLogger(scope);
      },
    },
    log(...args) {
      pushRecord("info", "runtime", args);
    },
    error(...args) {
      pushRecord("error", "runtime", args);
    },
    exit(code) {
      throw new Error(`plugin runtime exit(${code})`);
    },
  };
}

function createProcessingReactionController(config, loadedModule) {
  const addReaction = typeof loadedModule?.addReactionFeishu === "function" ? loadedModule.addReactionFeishu : null;
  const removeReaction =
    typeof loadedModule?.removeReactionFeishu === "function" ? loadedModule.removeReactionFeishu : null;
  const activeIndicators = new Map();

  async function start(params) {
    const messageId = readString(params?.messageId);
    const accountId = readString(params?.accountId) ?? "default";
    if (!messageId || !resolveTypingIndicatorEnabled(config, accountId) || !addReaction) {
      return false;
    }
    if (activeIndicators.has(messageId)) {
      return true;
    }
    try {
      const result = await addReaction({
        cfg: config,
        messageId,
        emojiType: "Typing",
        accountId,
      });
      activeIndicators.set(messageId, {
        reactionId: result?.reactionId ?? null,
        accountId,
      });
      emit("log", {
        level: "info",
        scope: "processing",
        message: `started typing reaction messageId=${messageId}`,
      });
      return true;
    } catch (error) {
      emit("log", {
        level: "warn",
        scope: "processing",
        message: `failed to start typing reaction messageId=${messageId}: ${error instanceof Error ? error.message : String(error)}`,
      });
      return false;
    }
  }

  async function stop(params) {
    const messageId = readString(params?.messageId);
    const accountId = readString(params?.accountId) ?? "default";
    if (!messageId) {
      return false;
    }
    const state = activeIndicators.get(messageId);
    activeIndicators.delete(messageId);
    if (!state?.reactionId || !removeReaction) {
      return false;
    }
    try {
      await removeReaction({
        cfg: config,
        messageId,
        reactionId: state.reactionId,
        accountId: state.accountId ?? accountId,
      });
      emit("log", {
        level: "info",
        scope: "processing",
        message: `stopped typing reaction messageId=${messageId}`,
      });
      return true;
    } catch (error) {
      emit("log", {
        level: "warn",
        scope: "processing",
        message: `failed to stop typing reaction messageId=${messageId}: ${error instanceof Error ? error.message : String(error)}`,
      });
      return false;
    }
  }

  return {
    start,
    stop,
  };
}

function normalizeLifecyclePhase(value) {
  const normalized = readString(value);
  if (!normalized) {
    return undefined;
  }
  const allowed = new Set([
    "reply_started",
    "processing_started",
    "ask_user_requested",
    "ask_user_answered",
    "approval_requested",
    "approval_resolved",
    "interrupt_requested",
    "resumed",
    "failed",
    "stopped",
    "tool_chunk_queued",
    "block_chunk_queued",
    "final_chunk_queued",
    "wait_for_idle",
    "idle_reached",
    "fully_complete",
    "dispatch_idle",
    "processing_stopped",
  ]);
  return allowed.has(normalized) ? normalized : undefined;
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
  const importMetaCompatBinding =
    "const __workclawImportMetaUrl = require('node:url').pathToFileURL(__filename).href;";

  while (stack.length > 0) {
    const currentDir = stack.pop();
    if (!currentDir) continue;

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
      const relativeShimCjsRoot = path.relative(path.dirname(entryPath), shimPluginSdkCjsRoot).replace(/\\/g, "/");
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
          /require\((['"])openclaw\/plugin-sdk(?:\/[^'"]+)?\1\)/g,
          (_match, quote) => `require(${quote}${relativeShimCjsRoot}/index.cjs${quote})`,
        )
        .replaceAll(
          /from\s+(['"])openclaw\/plugin-sdk(?:\/[^'"]+)?\1/g,
          (_match, quote) => `from ${quote}${relativeShimRoot}/index.js${quote}`,
        )
        .replaceAll(
          /import\s+(['"])openclaw\/plugin-sdk(?:\/[^'"]+)?\1/g,
          (_match, quote) => `import ${quote}${relativeShimRoot}/index.js${quote}`,
        )
        .replaceAll(/from\s+(['"])(\.\.?\/[^'"]+)\1/g, (_match, quote, specifier) => `from ${quote}${normalizeRelativeImport(specifier)}${quote}`)
        .replaceAll(/import\s+(['"])(\.\.?\/[^'"]+)\1/g, (_match, quote, specifier) => `import ${quote}${normalizeRelativeImport(specifier)}${quote}`);
      const needsImportMetaCompat =
        rewritten.includes("import.meta.url") &&
        ["module.exports", "exports.", "Object.defineProperty(exports"].some((marker) =>
          rewritten.includes(marker),
        );
      let normalized = needsImportMetaCompat
        ? rewritten
            .replaceAll(
              /const __filename = .*?import\.meta\.url.*?;/g,
              "const __filenameCompat = __filename;",
            )
            .replaceAll("dirname(__filename)", "dirname(__filenameCompat)")
            .replaceAll(".dirname)(__filename)", ".dirname)(__filenameCompat)")
            .replaceAll("import.meta.url", "__workclawImportMetaUrl")
        : rewritten;
      if (
        needsImportMetaCompat &&
        normalized.includes("__workclawImportMetaUrl") &&
        !normalized.includes(importMetaCompatBinding)
      ) {
        const strictDirectiveMatch = normalized.match(/^(["'])use strict\1;\r?\n?/);
        normalized = strictDirectiveMatch
          ? `${strictDirectiveMatch[0]}${importMetaCompatBinding}\n${normalized.slice(strictDirectiveMatch[0].length)}`
          : `${importMetaCompatBinding}\n${normalized}`;
      }
      fs.writeFileSync(entryPath, normalized, "utf8");
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

function resolveOutboundCommandSender(channel, config, defaultAccountId) {
  const outbound = asRecord(channel?.outbound);
  if (typeof outbound?.sendText !== "function") {
    throw new Error("feishu channel outbound.sendText not found");
  }
  let outboundCommandSequence = 0;

  return async function sendCommand(request) {
    const accountId = readString(request?.accountId) ?? defaultAccountId;
    const target = readString(request?.target);
    if (!target) {
      throw new Error("outbound target is required");
    }
    const text = readString(request?.text) ?? "";

    const result = await outbound.sendText({
      cfg: config,
      to: target,
      text,
      accountId,
      threadId: readString(request?.threadId),
    });
    const sequence = ++outboundCommandSequence;
    const normalized = asRecord(result);

    return {
      delivered: typeof normalized.delivered === "boolean" ? normalized.delivered : true,
      channel: readString(normalized.channel) ?? "feishu",
      accountId: readString(normalized.accountId) ?? accountId,
      target: readString(normalized.target) ?? target,
      threadId: readString(normalized.threadId) ?? readString(request?.threadId) ?? null,
      text: readString(normalized.text) ?? text,
      mode: readString(normalized.mode) ?? readString(request?.mode) ?? "text",
      messageId: readString(normalized.messageId) ?? `om_outbound_${sequence}`,
      chatId: readString(normalized.chatId) ?? target,
      sequence,
    };
  };
}

async function main() {
  installConsoleEventBridge();
  const args = parseArgs(process.argv.slice(2));
  const fixtureWorkspaceRoot = resolveFixtureWorkspaceRoot(args.fixtureRoot);
  const pluginRoot = path.resolve(args.pluginRoot);
  const preparedRoot = prepareFixture(pluginRoot, fixtureWorkspaceRoot, args.fixtureName);
  const manifest = resolvePluginManifest(preparedRoot);
  const entryPath = resolvePluginEntry(preparedRoot, manifest);
  const registry = createPluginRegistry();
  const configSource = args.configFile.trim() ? fs.readFileSync(path.resolve(args.configFile), "utf8") : args.configJson;
  const config = configSource.trim() ? JSON.parse(configSource) : {};
  const loadedModule = await import(pathToFileURL(entryPath).href);
  const processingReactions = createProcessingReactionController(config, loadedModule ?? null);
  const runtime = createPluginRuntime(config, { processingReactions });
  const logger = runtime.logging.getChildLogger({ scope: "plugin-host-feishu-runtime" });
  const api = createPluginApi(registry, {
    runtime,
    logger,
    config,
    registrationMode: normalizeRegistrationMode(manifest.registrationMode),
  });

  const plugin = resolvePluginExport(loadedModule);
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
  const sendOutboundCommand = resolveOutboundCommandSender(channel, config, args.accountId);

  const account = channel.config.resolveAccount(config, args.accountId);
  const runtimeStatus = {};
  const abortController = new AbortController();

  const shutdown = () => {
    if (!abortController.signal.aborted) {
      abortController.abort();
    }
  };

  process.on("SIGTERM", shutdown);
  process.on("SIGINT", shutdown);

  emit("ready", {
    pluginRoot,
    preparedRoot,
    entryPath,
    accountId: args.accountId,
  });

  let gatewayStartError = null;
  const gatewayStartPromise = channel.gateway
    .startAccount({
      cfg: config,
      accountId: args.accountId,
      account,
      runtime,
      abortSignal: abortController.signal,
      log: logger,
      getStatus: () => runtimeStatus,
      setStatus: (next) => {
        Object.assign(runtimeStatus, next);
        emit("status", { patch: next });
      },
      channelRuntime: runtime.channel,
    })
    .catch((error) => {
      gatewayStartError = error instanceof Error ? error : new Error(String(error));
      emit("fatal", {
        error: gatewayStartError.message,
      });
      if (!abortController.signal.aborted) {
        abortController.abort();
      }
    });

  const stdinInterface = readline.createInterface({
    input: process.stdin,
    crlfDelay: Infinity,
  });
  const pendingCommandChain = [];
  let commandChain = Promise.resolve();

  const handleCommandLine = async (line) => {
    const trimmed = String(line ?? "").trim();
    if (!trimmed) {
      return;
    }
    let commandPayload;
    try {
      commandPayload = normalizeCommandPayload(JSON.parse(trimmed));
    } catch (error) {
      emit("command_error", {
        requestId: null,
        command: null,
        error: error instanceof Error ? error.message : String(error),
      });
      return;
    }
    if (!commandPayload?.command) {
      emit("command_error", {
        requestId: commandPayload?.requestId ?? null,
        command: null,
        error: "command is required",
      });
      return;
    }
    if (commandPayload.command === "processing_stop") {
      if (!commandPayload.messageId) {
        emit("command_error", {
          requestId: commandPayload.requestId ?? null,
          command: commandPayload.command,
          error: "messageId is required",
        });
        return;
      }

      const accountId = commandPayload.accountId ?? args.accountId;
      const logicalReplyId = commandPayload.logicalReplyId ?? `reply_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
      await processingReactions.stop({
        messageId: commandPayload.messageId,
        accountId,
      });
      emitReplyLifecycle(
        {
          logicalReplyId,
          channel: "feishu",
          accountId,
          ...(commandPayload.threadId ? { threadId: commandPayload.threadId } : {}),
          messageId: commandPayload.messageId,
        },
        "processing_stopped",
        commandPayload.finalState ? { finalState: commandPayload.finalState } : {},
      );
      emit("processing_result", {
        requestId: commandPayload.requestId ?? null,
        command: commandPayload.command,
        accountId,
        messageId: commandPayload.messageId,
        logicalReplyId,
        ...(commandPayload.finalState ? { finalState: commandPayload.finalState } : {}),
      });
      return;
    }

    if (commandPayload.command === "lifecycle_event") {
      const phase = normalizeLifecyclePhase(commandPayload.phase);
      if (!phase) {
        emit("command_error", {
          requestId: commandPayload.requestId ?? null,
          command: commandPayload.command,
          error: "phase is required",
        });
        return;
      }

      const accountId = commandPayload.accountId ?? args.accountId;
      const logicalReplyId =
        commandPayload.logicalReplyId ?? `reply_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
      emitReplyLifecycle(
        {
          logicalReplyId,
          channel: "feishu",
          accountId,
          ...(commandPayload.threadId ? { threadId: commandPayload.threadId } : {}),
          ...(commandPayload.messageId ? { messageId: commandPayload.messageId } : {}),
        },
        phase,
      );
      emit("processing_result", {
        requestId: commandPayload.requestId ?? null,
        command: commandPayload.command,
        accountId,
        logicalReplyId,
        ...(commandPayload.messageId ? { messageId: commandPayload.messageId } : {}),
        phase,
      });
      return;
    }

    if (commandPayload.command !== "send_message") {
      emit("command_error", {
        requestId: commandPayload.requestId ?? null,
        command: commandPayload.command,
        error: `unsupported command: ${commandPayload.command}`,
      });
      return;
    }

    const request = {
      requestId: commandPayload.requestId ?? `cmd_${Date.now()}`,
      command: commandPayload.command,
      accountId: commandPayload.accountId ?? args.accountId,
      target: commandPayload.target,
      threadId: commandPayload.threadId,
      text: commandPayload.text ?? "",
      mode: commandPayload.mode ?? "text",
    };

    emit("send_request", {
      requestId: request.requestId,
      request,
    });
    try {
      const result = await sendOutboundCommand(request);
      emit("send_result", {
        requestId: request.requestId,
        request,
        result,
      });
    } catch (error) {
      emit("command_error", {
        requestId: request.requestId,
        command: request.command,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const lifecycle = new Promise((resolve) => {
    stdinInterface.on("line", (line) => {
      commandChain = commandChain
        .then(() => handleCommandLine(line))
        .catch((error) => {
          emit("command_error", {
            requestId: null,
            command: null,
            error: error instanceof Error ? error.message : String(error),
          });
        });
      pendingCommandChain.push(commandChain);
    });
    stdinInterface.on("close", () => {
      resolve("stdin-close");
    });
    abortController.signal.addEventListener(
      "abort",
      () => {
        resolve("abort");
      },
      { once: true },
    );
  });

  if (process.stdin.isTTY) {
    process.stdin.resume();
  }

  await lifecycle;
  await Promise.all(pendingCommandChain);
  await gatewayStartPromise;
  stdinInterface.close();

  if (gatewayStartError) {
    process.exitCode = 1;
    return;
  }

  emit("stopped", {
    accountId: args.accountId,
    runtimeStatus,
    logRecordCount: runtime.logging.records.length,
  });
}

main().catch((error) => {
  emit("fatal", {
    error: error instanceof Error ? error.message : String(error),
  });
  process.exitCode = 1;
});

process.on("unhandledRejection", (reason) => {
  emit("fatal", {
    error: reason instanceof Error ? reason.message : String(reason),
  });
  process.exitCode = 1;
});

process.on("uncaughtException", (error) => {
  emit("fatal", {
    error: error instanceof Error ? error.message : String(error),
  });
  process.exitCode = 1;
});
