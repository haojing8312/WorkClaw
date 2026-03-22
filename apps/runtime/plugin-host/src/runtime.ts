type LogLevel = "debug" | "info" | "warn" | "error";

export type RuntimeLogRecord = {
  level: LogLevel;
  scope: string;
  args: unknown[];
};

export type PluginRuntimeLogger = {
  debug?: (...args: unknown[]) => void;
  info?: (...args: unknown[]) => void;
  warn?: (...args: unknown[]) => void;
  error?: (...args: unknown[]) => void;
};

type PluginRuntimeConfig = Record<string, unknown>;
type AllowEntry = string | number;
type Authorizer = { configured: boolean; allowed: boolean };

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return value != null && typeof value === "object" ? (value as Record<string, unknown>) : undefined;
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function normalizeAllowFrom(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value
      .map((entry) => String(entry ?? "").trim())
      .filter(Boolean);
  }
  const scalar = readString(value);
  return scalar ? [scalar] : [];
}

function normalizeSenderId(senderId: string | undefined | null): string {
  return String(senderId ?? "").trim().toLowerCase();
}

function resolveFeishuConfig(config: PluginRuntimeConfig): Record<string, unknown> {
  return asRecord(asRecord(asRecord(config.channels)?.feishu) ?? {}) ?? {};
}

function resolveFeishuGroupConfig(config: PluginRuntimeConfig, groupId?: string | null): Record<string, unknown> | undefined {
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

function resolveRequireMention(config: PluginRuntimeConfig, groupId?: string | null): boolean {
  const groupConfig = resolveFeishuGroupConfig(config, groupId);
  const groupValue = groupConfig?.requireMention;
  if (typeof groupValue === "boolean") {
    return groupValue;
  }
  const feishu = resolveFeishuConfig(config);
  return feishu.requireMention === true;
}

function resolveGroupPolicy(config: PluginRuntimeConfig, groupId?: string | null): string {
  const groupConfig = resolveFeishuGroupConfig(config, groupId);
  return readString(groupConfig?.groupPolicy) ?? readString(resolveFeishuConfig(config).groupPolicy) ?? "allowlist";
}

function resolveEffectiveAllowFrom(config: PluginRuntimeConfig, accountId?: string | null): string[] {
  const feishu = resolveFeishuConfig(config);
  const accounts = asRecord(feishu.accounts);
  const accountKey = readString(accountId) ?? "default";
  const account = asRecord(accounts?.[accountKey]) ?? {};
  const merged = [
    ...normalizeAllowFrom(feishu.allowFrom),
    ...normalizeAllowFrom(account.allowFrom),
  ];
  return Array.from(new Set(merged));
}

function createSessionKey(channel: string, peerKind: string, peerId: string, agentId: string): string {
  const normalizedPeer = peerId.trim().toLowerCase() || "unknown";
  return `agent:${agentId}:${channel}:${peerKind}:${normalizedPeer}`;
}

function resolveAgentRoute(params: {
  channel?: string;
  peer?: { kind?: string; id?: string };
}): { agentId: string; sessionKey: string; matchedBy: string } {
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

type ReplyDispatchKind = "tool" | "block" | "final";

function createReplyDispatcher() {
  let sendChain: Promise<void> = Promise.resolve();
  let completeCalled = false;
  const queuedCounts: Record<ReplyDispatchKind, number> = {
    tool: 0,
    block: 0,
    final: 0,
  };

  const enqueue = (kind: ReplyDispatchKind) => {
    queuedCounts[kind] += 1;
    sendChain = sendChain.then(async () => undefined);
    return true;
  };

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
      await sendChain;
    },
    getQueuedCounts() {
      return { ...queuedCounts };
    },
    markComplete() {
      completeCalled = true;
    },
    isComplete() {
      return completeCalled;
    },
  };
}

function stripPrefixedTarget(rawTarget: unknown, prefix: string): string | undefined {
  const value = readString(rawTarget);
  if (!value) {
    return undefined;
  }
  if (value.startsWith(prefix)) {
    return value.slice(prefix.length).trim() || undefined;
  }
  return undefined;
}

function deriveDispatchThreadId(ctx: Record<string, unknown>): string {
  const chatType = readString(ctx.ChatType) ?? "direct";
  return (
    readString(ctx.GroupSubject) ??
    stripPrefixedTarget(ctx.To, "chat:") ??
    readString(ctx.ChatId) ??
    stripPrefixedTarget(ctx.To, "user:") ??
    stripPrefixedTarget(ctx.From, "feishu:") ??
    (chatType === "group" ? readString(ctx.ChatId) : readString(ctx.SenderId)) ??
    (chatType === "group" ? "unknown-group" : "unknown-user")
  );
}

function generatePairingCode(): string {
  return Math.random().toString(36).slice(2, 10).toUpperCase();
}

function normalizeOutboundTarget(rawTarget: unknown): string | undefined {
  return (
    stripPrefixedTarget(rawTarget, "chat:") ??
    stripPrefixedTarget(rawTarget, "user:") ??
    stripPrefixedTarget(rawTarget, "feishu:") ??
    readString(rawTarget)
  );
}

export type PluginRuntimeState = {
  config: {
    loadConfig: () => PluginRuntimeConfig;
  };
  channel: {
    text: {
      chunkMarkdownText: (text: string, limit: number) => string[];
      chunkTextWithMode: (text: string, limit: number, mode?: string) => string[];
      resolveTextChunkLimit: (cfg?: PluginRuntimeConfig, fallbackLimit?: number) => number;
      resolveChunkMode: (cfg?: PluginRuntimeConfig, fallbackMode?: string) => string;
      resolveMarkdownTableMode: (cfg?: PluginRuntimeConfig, fallbackMode?: string) => string;
      convertMarkdownTables: (text: string, _mode: string) => string;
    };
    groups: {
      resolveGroupPolicy: (params: { cfg?: PluginRuntimeConfig; groupId?: string | null }) => string;
      resolveRequireMention: (params: { cfg?: PluginRuntimeConfig; groupId?: string | null }) => boolean;
    };
    pairing: {
      readAllowFromStore: (params?: { cfg?: PluginRuntimeConfig; accountId?: string | null }) => Promise<string[]>;
      upsertPairingRequest: (params: {
        channel?: string;
        id?: string;
        accountId?: string | null;
        meta?: Record<string, unknown>;
      }) => Promise<Record<string, unknown>>;
      buildPairingReply: (params: { channel?: string; idLine?: string; code?: string }) => string;
    };
    commands: {
      shouldComputeCommandAuthorized: (rawBody: string, _cfg?: PluginRuntimeConfig) => boolean;
      resolveCommandAuthorizedFromAuthorizers: (params: {
        useAccessGroups?: boolean;
        authorizers?: Authorizer[];
      }) => boolean;
      isControlCommandMessage: (rawBody: string) => boolean;
    };
    routing: {
      resolveAgentRoute: (params: {
        channel?: string;
        peer?: { kind?: string; id?: string };
      }) => { agentId: string; sessionKey: string; matchedBy: string };
    };
    outbound: {
      sendMessage: (params: {
        accountId?: string | null;
        target?: string | null;
        threadId?: string | null;
        text?: string | null;
        mode?: string | null;
      }) => Promise<{
        delivered: boolean;
        channel: string;
        accountId: string;
        target: string;
        threadId?: string;
        text: string;
        mode: string;
        messageId: string;
        chatId: string;
        sequence: number;
      }>;
    };
    reply: {
        resolveEnvelopeFormatOptions: (_cfg?: PluginRuntimeConfig) => Record<string, unknown>;
        formatAgentEnvelope: (params: { body?: string; bodyForAgent?: string }) => string;
        finalizeInboundContext: (ctx: Record<string, unknown>) => Record<string, unknown>;
        createReplyDispatcherWithTyping: () => {
          dispatcher: ReturnType<typeof createReplyDispatcher>;
          replyOptions: Record<string, unknown>;
          markDispatchIdle: () => void;
          markRunComplete: () => void;
        };
        resolveHumanDelayConfig: () => { enabled: false; delayMs: 0 };
        withReplyDispatcher: (params: {
          dispatcher: ReturnType<typeof createReplyDispatcher>;
          run: () => Promise<unknown>;
          onSettled?: () => void | Promise<void>;
        }) => Promise<unknown>;
        dispatchReplyFromConfig: (_params: Record<string, unknown>) => Promise<{
          queuedFinal: boolean;
          counts: { final: number };
      }>;
      dispatchReplyWithBufferedBlockDispatcher: (_params: Record<string, unknown>) => Promise<{
        delivered: boolean;
      }>;
    };
    media: {
      saveMediaBuffer: (params: { fileName?: string; buffer?: Uint8Array | ArrayBuffer }) => Promise<string>;
    };
  };
  system: {
    records: Array<{ message: string; meta?: Record<string, unknown> }>;
    dispatchRequests: Array<Record<string, unknown>>;
    enqueueSystemEvent: (message: string, meta?: Record<string, unknown>) => void;
  };
  logging: {
    records: RuntimeLogRecord[];
    getChildLogger: (input: { scope: string }) => PluginRuntimeLogger;
  };
  log: (...args: unknown[]) => void;
  error: (...args: unknown[]) => void;
  exit: (code: number) => never;
};

export function createPluginRuntime(input: {
  config?: PluginRuntimeConfig;
}): PluginRuntimeState {
  const records: RuntimeLogRecord[] = [];
  const systemRecords: Array<{ message: string; meta?: Record<string, unknown> }> = [];
  const dispatchRequests: Array<Record<string, unknown>> = [];
  const pairingRequests = new Map<string, Record<string, unknown>>();
  let outboundSendSequence = 0;
  const config = input.config ?? {};

  function createLogger(scope: string): PluginRuntimeLogger {
    return {
      debug: (...args) => records.push({ level: "debug", scope, args }),
      info: (...args) => records.push({ level: "info", scope, args }),
      warn: (...args) => records.push({ level: "warn", scope, args }),
      error: (...args) => records.push({ level: "error", scope, args }),
    };
  }

  return {
    config: {
      loadConfig: () => config,
    },
    channel: {
      text: {
        chunkMarkdownText(text, limit) {
          if (limit <= 0) {
            return [text];
          }
          const chunks: string[] = [];
          for (let index = 0; index < text.length; index += limit) {
            chunks.push(text.slice(index, index + limit));
          }
          return chunks;
        },
        chunkTextWithMode(text, limit) {
          if (limit <= 0) {
            return [text];
          }
          const chunks: string[] = [];
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
          if (!configured) {
            return true;
          }
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
          const accountId = readString(params.accountId) ?? "default";
          const target = normalizeOutboundTarget(params.target);
          if (!target) {
            throw new Error("outbound target is required");
          }
          const text = readString(params.text) ?? "";
          const mode = readString(params.mode) ?? "text";
          const threadId = readString(params.threadId);
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
          const dispatcher = createReplyDispatcher();
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
          return { enabled: false as const, delayMs: 0 };
        },
        async withReplyDispatcher(params) {
          try {
            return await params.run();
          } finally {
            params.dispatcher.markComplete();
            try {
              await params.dispatcher.waitForIdle();
            } finally {
              await params.onSettled?.();
            }
          }
        },
        async dispatchReplyFromConfig(params) {
          const ctx = asRecord(params.ctx) ?? {};
          const chatId =
            stripPrefixedTarget(ctx.To, "chat:") ??
            readString(ctx.ChatId) ??
            undefined;
          dispatchRequests.push({
            accountId: readString(ctx.AccountId) ?? "default",
            threadId: deriveDispatchThreadId(ctx),
            ...(chatId ? { chatId } : {}),
            senderId: readString(ctx.SenderId) ?? "",
            messageId: readString(ctx.MessageSid),
            text:
              readString(ctx.RawBody) ??
              readString(ctx.CommandBody) ??
              readString(ctx.BodyForAgent) ??
              readString(ctx.Body) ??
              "",
            chatType: readString(ctx.ChatType) ?? "direct",
          });
          return {
            queuedFinal: false,
            counts: { final: 0 },
          };
        },
        async dispatchReplyWithBufferedBlockDispatcher() {
          return { delivered: false };
        },
      },
      media: {
        async saveMediaBuffer(params) {
          const fileName = readString(params.fileName) ?? `media-${Date.now()}`;
          return fileName;
        },
      },
    },
    system: {
      records: systemRecords,
      dispatchRequests,
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
      records.push({ level: "info", scope: "runtime", args });
    },
    error(...args) {
      records.push({ level: "error", scope: "runtime", args });
    },
    exit(code) {
      throw new Error(`plugin runtime exit(${code})`);
    },
  };
}
