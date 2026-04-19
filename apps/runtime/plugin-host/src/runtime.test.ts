import fs from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { createInterface } from "node:readline";
import { afterEach, describe, expect, it } from "vitest";
import { createPluginRuntime } from "./runtime";
const tempRoots: string[] = [];
const runtimeWorkspaceDir = process.cwd();
const pluginHostDir = path.join(runtimeWorkspaceDir, "plugin-host");
const tempBaseDir = path.join(pluginHostDir, ".tmp-tests");

async function createTempPluginRoot(): Promise<string> {
  await fs.mkdir(tempBaseDir, { recursive: true });
  const root = await fs.mkdtemp(path.join(tempBaseDir, "workclaw-plugin-host-"));
  tempRoots.push(root);
  return root;
}

afterEach(async () => {
  await Promise.all(tempRoots.splice(0, tempRoots.length).map((root) => fs.rm(root, { recursive: true, force: true })));
});

function parseJsonLines(text: string): Array<Record<string, unknown>> {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .filter((line) => line.startsWith("{"))
    .map((line) => JSON.parse(line) as Record<string, unknown>);
}

function createEventCollector(stdout: NodeJS.ReadableStream, stderr: NodeJS.ReadableStream) {
  const events: Array<Record<string, unknown>> = [];
  const stderrLines: string[] = [];
  const waiters: Array<{
    predicate: (event: Record<string, unknown>) => boolean;
    resolve: (event: Record<string, unknown>) => void;
    reject: (error: Error) => void;
    timeout: NodeJS.Timeout;
  }> = [];
  const stdoutInterface = createInterface({ input: stdout });

  const settleWaiters = (event: Record<string, unknown>) => {
    for (let index = waiters.length - 1; index >= 0; index -= 1) {
      const waiter = waiters[index];
      if (!waiter.predicate(event)) {
        continue;
      }
      clearTimeout(waiter.timeout);
      waiters.splice(index, 1);
      waiter.resolve(event);
    }
  };

  stdoutInterface.on("line", (line) => {
    const trimmed = line.trim();
    if (!trimmed || !trimmed.startsWith("{")) {
      return;
    }
    const event = JSON.parse(trimmed) as Record<string, unknown>;
    events.push(event);
    settleWaiters(event);
  });

  stderr.on("data", (chunk) => {
    stderrLines.push(chunk.toString("utf8"));
  });

  return {
    events,
    stderrLines,
    waitFor(
      predicate: (event: Record<string, unknown>) => boolean,
      message: string,
      timeoutMs = 15_000,
    ): Promise<Record<string, unknown>> {
      const existing = events.find(predicate);
      if (existing) {
        return Promise.resolve(existing);
      }
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          const stderrText = stderrLines.join("");
          reject(new Error(`${message}\n${stderrText ? `stderr:\n${stderrText}` : "stderr: <empty>"}`));
        }, timeoutMs);
        waiters.push({ predicate, resolve, reject, timeout });
      });
    },
    close() {
      stdoutInterface.close();
      for (const waiter of waiters.splice(0, waiters.length)) {
        clearTimeout(waiter.timeout);
        waiter.reject(new Error("collector closed"));
      }
    },
  };
}

describe("plugin runtime", () => {
  it("provides logger hierarchy, config loader, and channel compatibility helpers", () => {
    const runtime = createPluginRuntime({
      config: {
        channels: {
          feishu: {
            enabled: true,
            requireMention: true,
            groupPolicy: "allowlist",
            allowFrom: ["ou_owner"],
          },
        },
      },
    });

    const child = runtime.logging.getChildLogger({ scope: "feishu" });
    child.info?.("hello");
    runtime.system.enqueueSystemEvent("inbound", { sessionKey: "agent:main:direct:user" });

    expect(runtime.channel.text.chunkMarkdownText("abcdef", 3)).toEqual(["abc", "def"]);
    expect(runtime.channel.text.convertMarkdownTables("|a|", "bullets")).toBe("|a|");
    expect(runtime.channel.groups.resolveRequireMention({})).toBe(true);
    expect(runtime.channel.groups.resolveGroupPolicy({})).toBe("allowlist");
    expect(
      runtime.channel.pairing.buildPairingReply({
        channel: "feishu",
        idLine: "ou_sender",
        code: "PAIR123",
      }),
    ).toContain("openclaw pairing approve feishu PAIR123");
    expect(runtime.channel.commands.shouldComputeCommandAuthorized("/help", {})).toBe(true);
    expect(
      runtime.channel.routing.resolveAgentRoute({
        channel: "feishu",
        peer: { kind: "direct", id: "ou_user" },
      }).sessionKey,
    ).toContain("feishu:direct:ou_user");
    expect(runtime.config.loadConfig()).toEqual({
      channels: {
        feishu: {
          enabled: true,
          requireMention: true,
          groupPolicy: "allowlist",
          allowFrom: ["ou_owner"],
        },
      },
    });
    expect(runtime.logging.records).toHaveLength(1);
    expect(runtime.logging.records[0]?.scope).toBe("feishu");
    expect(runtime.system.records).toHaveLength(1);
  });

  it("captures dispatch requests from the official reply bridge", async () => {
    const runtime = createPluginRuntime({ config: {} });
    const reply = runtime.channel.reply.createReplyDispatcherWithTyping();

    const dispatchResult = await runtime.channel.reply.dispatchReplyFromConfig({
      ctx: {
        AccountId: "default",
        SenderId: "ou_sender",
        MessageSid: "om_123",
        RawBody: "你好",
        ChatType: "direct",
        ChatId: "oc_chat_123",
        To: "user:ou_sender",
        From: "feishu:ou_sender",
      },
      dispatcher: reply.dispatcher,
    });

    expect(dispatchResult).toEqual({
      queuedFinal: false,
      counts: { final: 0 },
    });

    expect(runtime.system.dispatchRequests).toEqual([
      {
        accountId: "default",
        chatId: "oc_chat_123",
        threadId: "oc_chat_123",
        senderId: "ou_sender",
        messageId: "om_123",
        text: "你好",
        chatType: "direct",
      },
    ]);
    expect(runtime.system.replyLifecycleEvents.map((event) => event.phase)).toEqual([
      "reply_started",
      "processing_started",
    ]);
  });

  it("matches the reply dispatcher shape expected by the official feishu plugin", async () => {
    const runtime = createPluginRuntime({ config: {} });
    const result = runtime.channel.reply.createReplyDispatcherWithTyping();

    expect(result.replyOptions).toEqual({});
    expect(typeof result.markDispatchIdle).toBe("function");
    expect(typeof result.markRunComplete).toBe("function");
    expect(typeof result.dispatcher.sendToolResult).toBe("function");
    expect(typeof result.dispatcher.sendBlockReply).toBe("function");
    expect(typeof result.dispatcher.sendFinalReply).toBe("function");
    expect(typeof result.dispatcher.waitForIdle).toBe("function");
    expect(result.dispatcher.getQueuedCounts()).toEqual({
      tool: 0,
      block: 0,
      final: 0,
    });

    await runtime.channel.reply.withReplyDispatcher({
      dispatcher: result.dispatcher,
      run: async () => {
        result.dispatcher.sendFinalReply({ text: "ok" });
      },
    });

    result.markDispatchIdle();

    await expect(result.dispatcher.waitForIdle()).resolves.toBeUndefined();
    expect(runtime.system.replyLifecycleEvents.map((event) => event.phase)).toEqual([
      "final_chunk_queued",
      "wait_for_idle",
      "idle_reached",
      "fully_complete",
      "dispatch_idle",
    ]);
  });

  it("treats dispatch_idle as the terminal lifecycle barrier after fully_complete", async () => {
    const runtime = createPluginRuntime({ config: {} });
    const result = runtime.channel.reply.createReplyDispatcherWithTyping();

    await runtime.channel.reply.withReplyDispatcher({
      dispatcher: result.dispatcher,
      run: async () => {
        result.dispatcher.sendFinalReply({ text: "ok" });
      },
    });

    const phasesBeforeManualIdle = runtime.system.replyLifecycleEvents.map((event) => event.phase);
    const waitIndex = phasesBeforeManualIdle.indexOf("wait_for_idle");
    const idleReachedIndex = phasesBeforeManualIdle.indexOf("idle_reached");
    const fullyCompleteIndex = phasesBeforeManualIdle.indexOf("fully_complete");
    const dispatchIdleIndex = phasesBeforeManualIdle.indexOf("dispatch_idle");

    expect(waitIndex).toBeGreaterThanOrEqual(0);
    expect(idleReachedIndex).toBeGreaterThan(waitIndex);
    expect(fullyCompleteIndex).toBeGreaterThan(idleReachedIndex);
    expect(dispatchIdleIndex).toBeGreaterThan(fullyCompleteIndex);

    result.markDispatchIdle();

    expect(runtime.system.replyLifecycleEvents.map((event) => event.phase)).toEqual(phasesBeforeManualIdle);
    expect(phasesBeforeManualIdle.filter((phase) => phase === "dispatch_idle")).toHaveLength(1);
  });

  it("routes outbound send commands through the official runtime fixture", async () => {
    const pluginRoot = await createTempPluginRoot();
    await fs.writeFile(
      path.join(pluginRoot, "package.json"),
      JSON.stringify({
        type: "module",
        openclaw: {
          extensions: ["./index.js"],
        },
      }),
      "utf8",
    );
    await fs.writeFile(
      path.join(pluginRoot, "index.js"),
      [
        "export default {",
        "  register(api) {",
        "    api.registerChannel({",
        "      plugin: {",
        "        id: 'feishu',",
        "        config: {",
        "          resolveAccount() {",
        "            return { accountId: 'default', enabled: true, configured: true };",
        "          },",
        "        },",
        "        outbound: {",
        "          async sendText({ to, text, accountId, threadId }) {",
        "            return {",
        "              channel: 'feishu',",
        "              delivered: true,",
        "              accountId,",
        "              target: to,",
        "              text,",
        "              threadId: threadId ?? null,",
        "              chatId: `plugin:${to}`,",
        "              messageId: 'plugin_message_1',",
        "            };",
        "          },",
        "        },",
        "        gateway: {",
        "          async startAccount({ setStatus }) {",
        "            setStatus({ running: true });",
        "          },",
        "        },",
        "      },",
        "    });",
        "  },",
        "};",
      ].join("\n"),
      "utf8",
    );

    const sendCommand = {
      requestId: "request-1",
      command: "send_message",
      accountId: "default",
      target: "oc_chat_123",
      text: "你好",
      mode: "text",
    };

    const child = spawn(
      process.execPath,
      [
        path.join("scripts", "run-feishu-host.mjs"),
        "--plugin-root",
        pluginRoot,
        "--fixture-name",
        "runtime-outbound-send",
        "--account-id",
        "default",
      ],
      {
        cwd: pluginHostDir,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

    const collector = createEventCollector(child.stdout, child.stderr);
    try {
      const readyEvent = await collector.waitFor(
        (event) => event.event === "ready",
        "expected ready event from runtime fixture",
      );
      expect(readyEvent).toMatchObject({
        event: "ready",
        accountId: "default",
      });

      child.stdin.write(`${JSON.stringify(sendCommand)}\n`);

      const sendResultEvent = await collector.waitFor(
        (event) => event.event === "send_result" && event.requestId === "request-1",
        "expected send_result event from runtime fixture",
      );
      expect(sendResultEvent).toMatchObject({
        event: "send_result",
        requestId: "request-1",
        request: expect.objectContaining({
          requestId: "request-1",
          command: "send_message",
          accountId: "default",
          target: "oc_chat_123",
          text: "你好",
          mode: "text",
        }),
        result: expect.objectContaining({
          delivered: true,
          channel: "feishu",
          accountId: "default",
          target: "oc_chat_123",
          text: "你好",
          mode: "text",
          threadId: null,
          chatId: "plugin:oc_chat_123",
          messageId: "plugin_message_1",
          sequence: 1,
        }),
      });

      child.stdin.end();
      await collector.waitFor(
        (event) => event.event === "stopped",
        "expected stopped event after stdin close",
      );
      await once(child, "exit");
    } finally {
      collector.close();
      child.kill();
    }
  });

  it("accepts outbound send commands even when gateway.startAccount keeps the runtime alive", async () => {
    const pluginRoot = await createTempPluginRoot();
    await fs.writeFile(
      path.join(pluginRoot, "package.json"),
      JSON.stringify({
        type: "module",
        openclaw: {
          extensions: ["./index.js"],
        },
      }),
      "utf8",
    );
    await fs.writeFile(
      path.join(pluginRoot, "index.js"),
      [
        "export default {",
        "  register(api) {",
        "    api.registerChannel({",
        "      plugin: {",
        "        id: 'feishu',",
        "        config: {",
        "          resolveAccount() {",
        "            return { accountId: 'default', enabled: true, configured: true };",
        "          },",
        "        },",
        "        outbound: {",
        "          async sendText({ to, text, accountId }) {",
        "            return {",
        "              channel: 'feishu',",
        "              delivered: true,",
        "              accountId,",
        "              target: to,",
        "              text,",
        "              chatId: `plugin:${to}`,",
        "              messageId: 'plugin_message_lived_1',",
        "            };",
        "          },",
        "        },",
        "        gateway: {",
        "          async startAccount({ setStatus, abortSignal }) {",
        "            setStatus({ running: true });",
            "            await new Promise((resolve) => {",
            "              abortSignal.addEventListener('abort', resolve, { once: true });",
        "            });",
        "          },",
        "        },",
        "      },",
        "    });",
        "  },",
        "};",
      ].join("\n"),
      "utf8",
    );

    const sendCommand = {
      requestId: "request-lived-1",
      command: "send_message",
      accountId: "default",
      target: "oc_chat_123",
      text: "你好，常驻运行时",
      mode: "text",
    };

    const child = spawn(
      process.execPath,
      [
        path.join("scripts", "run-feishu-host.mjs"),
        "--plugin-root",
        pluginRoot,
        "--fixture-name",
        "runtime-long-lived-outbound-send",
        "--account-id",
        "default",
      ],
      {
        cwd: pluginHostDir,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

    const collector = createEventCollector(child.stdout, child.stderr);
    try {
      await collector.waitFor(
        (event) => event.event === "ready",
        "expected ready event from long-lived runtime fixture",
      );

      child.stdin.write(`${JSON.stringify(sendCommand)}\n`);

      const sendResultEvent = await collector.waitFor(
        (event) => event.event === "send_result" && event.requestId === "request-lived-1",
        "expected send_result event from long-lived runtime fixture",
      );
      expect(sendResultEvent).toMatchObject({
        event: "send_result",
        requestId: "request-lived-1",
        result: expect.objectContaining({
          delivered: true,
          channel: "feishu",
          accountId: "default",
          target: "oc_chat_123",
          text: "你好，常驻运行时",
          mode: "text",
          threadId: null,
          chatId: "plugin:oc_chat_123",
          messageId: "plugin_message_lived_1",
          sequence: 1,
        }),
      });

      child.kill();
      await once(child, "exit");
    } finally {
      collector.close();
      child.kill();
    }
  });

  it("starts and stops the official processing reaction through the host command bridge", async () => {
    const pluginRoot = await createTempPluginRoot();
    await fs.writeFile(
      path.join(pluginRoot, "package.json"),
      JSON.stringify({
        type: "module",
        openclaw: {
          extensions: ["./index.js"],
        },
      }),
      "utf8",
    );
    await fs.writeFile(
      path.join(pluginRoot, "index.js"),
      [
        "export async function addReactionFeishu({ messageId }) {",
        "  return { reactionId: `typing_${messageId}` };",
        "}",
        "export async function removeReactionFeishu() {",
        "  return { removed: true };",
        "}",
        "export default {",
        "  register(api) {",
        "    api.registerChannel({",
        "      plugin: {",
        "        id: 'feishu',",
        "        config: {",
        "          resolveAccount() {",
        "            return { accountId: 'default', enabled: true, configured: true };",
        "          },",
        "        },",
        "        outbound: {",
        "          async sendText({ to, text, accountId }) {",
        "            return { delivered: true, channel: 'feishu', accountId, target: to, text, chatId: `plugin:${to}`, messageId: 'plugin_processing_message_1' };",
        "          },",
        "        },",
        "        gateway: {",
        "          async startAccount({ runtime, setStatus, abortSignal }) {",
        "            setStatus({ running: true, processingFixture: true });",
        "            const reply = runtime.channel.reply.createReplyDispatcherWithTyping();",
        "            await runtime.channel.reply.dispatchReplyFromConfig({",
        "              ctx: {",
        "                AccountId: 'default',",
        "                SenderId: 'ou_sender_processing',",
        "                MessageSid: 'om_processing_123',",
        "                RawBody: '请处理一下',",
        "                ChatType: 'direct',",
        "                ChatId: 'oc_processing_chat_123',",
        "                To: 'user:ou_sender_processing',",
        "                From: 'feishu:ou_sender_processing',",
        "              },",
        "              dispatcher: reply.dispatcher,",
        "            });",
        "            await new Promise((resolve) => abortSignal.addEventListener('abort', resolve, { once: true }));",
        "          },",
        "        },",
        "      },",
        "    });",
        "  },",
        "};",
      ].join("\n"),
      "utf8",
    );

    const child = spawn(
      process.execPath,
      [
        path.join("scripts", "run-feishu-host.mjs"),
        "--plugin-root",
        pluginRoot,
        "--fixture-name",
        "runtime-processing-reaction-bridge",
        "--account-id",
        "default",
      ],
      {
        cwd: pluginHostDir,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

    const collector = createEventCollector(child.stdout, child.stderr);
    try {
      await collector.waitFor(
        (event) => event.event === "ready",
        "expected ready event from processing reaction fixture",
      );

      const replyStarted = await collector.waitFor(
        (event) => event.event === "reply_lifecycle" && event.phase === "reply_started",
        "expected reply_started lifecycle event from processing reaction fixture",
      );
      expect(replyStarted).toMatchObject({
        event: "reply_lifecycle",
        phase: "reply_started",
        accountId: "default",
        messageId: "om_processing_123",
      });

      const processingStarted = await collector.waitFor(
        (event) => event.event === "reply_lifecycle" && event.phase === "processing_started",
        "expected processing_started lifecycle event from processing reaction fixture",
      );
      expect(processingStarted).toMatchObject({
        event: "reply_lifecycle",
        phase: "processing_started",
        accountId: "default",
        messageId: "om_processing_123",
      });

      const startTypingLog = await collector.waitFor(
        (event) =>
          event.event === "log" &&
          event.scope === "processing" &&
          String(event.message ?? "").includes("started typing reaction"),
        "expected typing reaction start log",
      );
      expect(startTypingLog).toMatchObject({
        event: "log",
        level: "info",
        scope: "processing",
      });

      child.stdin.write(
        `${JSON.stringify({
          requestId: "processing-stop-1",
          command: "processing_stop",
          accountId: "default",
          messageId: "om_processing_123",
          logicalReplyId: replyStarted.logicalReplyId,
          finalState: "completed",
        })}\n`,
      );

      const processingStopped = await collector.waitFor(
        (event) => event.event === "reply_lifecycle" && event.phase === "processing_stopped",
        "expected processing_stopped lifecycle event after processing stop",
      );
      expect(processingStopped).toMatchObject({
        event: "reply_lifecycle",
        phase: "processing_stopped",
        finalState: "completed",
      });
      expect(
        collector.events.filter(
          (event) =>
            event.event === "reply_lifecycle" &&
            (event.phase === "fully_complete" || event.phase === "dispatch_idle"),
        ),
      ).toHaveLength(0);

      const stopTypingLog = await collector.waitFor(
        (event) =>
          event.event === "log" &&
          event.scope === "processing" &&
          String(event.message ?? "").includes("stopped typing reaction"),
        "expected typing reaction stop log",
      );
      expect(stopTypingLog).toMatchObject({
        event: "log",
        level: "info",
        scope: "processing",
      });

      const processingResult = await collector.waitFor(
        (event) => event.event === "processing_result" && event.requestId === "processing-stop-1",
        "expected processing_result event after processing stop",
      );
      expect(processingResult).toMatchObject({
        event: "processing_result",
        requestId: "processing-stop-1",
        command: "processing_stop",
        accountId: "default",
        messageId: "om_processing_123",
        logicalReplyId: replyStarted.logicalReplyId,
        finalState: "completed",
      });

      child.kill();
      await once(child, "exit");
    } finally {
      collector.close();
      child.kill();
    }
  });

  it("emits custom lifecycle phases through the host command bridge", async () => {
    const pluginRoot = await createTempPluginRoot();
    await fs.writeFile(
      path.join(pluginRoot, "package.json"),
      JSON.stringify({
        type: "module",
        openclaw: {
          extensions: ["./index.js"],
        },
      }),
      "utf8",
    );
    await fs.writeFile(
      path.join(pluginRoot, "index.js"),
      [
        "export default {",
        "  register(api) {",
        "    api.registerChannel({",
        "      plugin: {",
        "        id: 'feishu',",
        "        config: {",
        "          resolveAccount() {",
        "            return { accountId: 'default', enabled: true, configured: true };",
        "          },",
        "        },",
        "        outbound: {",
        "          async sendText({ to, text, accountId }) {",
        "            return { delivered: true, channel: 'feishu', accountId, target: to, text, chatId: `plugin:${to}`, messageId: 'plugin_custom_lifecycle_message_1' };",
        "          },",
        "        },",
        "        gateway: {",
        "          async startAccount({ setStatus, abortSignal }) {",
        "            setStatus({ running: true, customLifecycleFixture: true });",
        "            await new Promise((resolve) => abortSignal.addEventListener('abort', resolve, { once: true }));",
        "          },",
        "        },",
        "      },",
        "    });",
        "  },",
        "};",
      ].join("\n"),
      "utf8",
    );

    const child = spawn(
      process.execPath,
      [
        path.join("scripts", "run-feishu-host.mjs"),
        "--plugin-root",
        pluginRoot,
        "--fixture-name",
        "runtime-custom-lifecycle-bridge",
        "--account-id",
        "default",
      ],
      {
        cwd: pluginHostDir,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

    const collector = createEventCollector(child.stdout, child.stderr);
    try {
      await collector.waitFor(
        (event) => event.event === "ready",
        "expected ready event from custom lifecycle fixture",
      );

      child.stdin.write(
        `${JSON.stringify({
          requestId: "lifecycle-1",
          command: "lifecycle_event",
          accountId: "default",
          logicalReplyId: "reply_custom_1",
          threadId: "oc_chat_custom_1",
          messageId: "om_custom_1",
          phase: "ask_user_requested",
        })}\n`,
      );

      const lifecycleEvent = await collector.waitFor(
        (event) => event.event === "reply_lifecycle" && event.phase === "ask_user_requested",
        "expected ask_user_requested lifecycle event from custom lifecycle bridge",
      );
      expect(lifecycleEvent).toMatchObject({
        event: "reply_lifecycle",
        phase: "ask_user_requested",
        logicalReplyId: "reply_custom_1",
        accountId: "default",
        threadId: "oc_chat_custom_1",
        messageId: "om_custom_1",
      });

      const commandResult = await collector.waitFor(
        (event) => event.event === "processing_result" && event.requestId === "lifecycle-1",
        "expected processing_result event for custom lifecycle bridge",
      );
      expect(commandResult).toMatchObject({
        event: "processing_result",
        requestId: "lifecycle-1",
        command: "lifecycle_event",
        phase: "ask_user_requested",
      });

      child.stdin.write(
        `${JSON.stringify({
          requestId: "lifecycle-2",
          command: "lifecycle_event",
          accountId: "default",
          logicalReplyId: "reply_custom_1",
          threadId: "oc_chat_custom_1",
          messageId: "om_custom_1",
          phase: "failed",
        })}\n`,
      );

      const failedEvent = await collector.waitFor(
        (event) => event.event === "reply_lifecycle" && event.phase === "failed",
        "expected failed lifecycle event from custom lifecycle bridge",
      );
      expect(failedEvent).toMatchObject({
        event: "reply_lifecycle",
        phase: "failed",
        logicalReplyId: "reply_custom_1",
      });

      child.stdin.write(
        `${JSON.stringify({
          requestId: "lifecycle-3",
          command: "lifecycle_event",
          accountId: "default",
          logicalReplyId: "reply_custom_1",
          threadId: "oc_chat_custom_1",
          messageId: "om_custom_1",
          phase: "stopped",
        })}\n`,
      );

      const stoppedEvent = await collector.waitFor(
        (event) => event.event === "reply_lifecycle" && event.phase === "stopped",
        "expected stopped lifecycle event from custom lifecycle bridge",
      );
      expect(stoppedEvent).toMatchObject({
        event: "reply_lifecycle",
        phase: "stopped",
        logicalReplyId: "reply_custom_1",
      });

      child.stdin.write(
        `${JSON.stringify({
          requestId: "lifecycle-4",
          command: "lifecycle_event",
          accountId: "default",
          logicalReplyId: "reply_custom_1",
          threadId: "oc_chat_custom_1",
          messageId: "om_custom_1",
          phase: "ask_user_answered",
        })}\n`,
      );

      const askUserAnsweredEvent = await collector.waitFor(
        (event) => event.event === "reply_lifecycle" && event.phase === "ask_user_answered",
        "expected ask_user_answered lifecycle event from custom lifecycle bridge",
      );
      expect(askUserAnsweredEvent).toMatchObject({
        event: "reply_lifecycle",
        phase: "ask_user_answered",
        logicalReplyId: "reply_custom_1",
      });

      child.stdin.write(
        `${JSON.stringify({
          requestId: "lifecycle-5",
          command: "lifecycle_event",
          accountId: "default",
          logicalReplyId: "reply_custom_1",
          threadId: "oc_chat_custom_1",
          messageId: "om_custom_1",
          phase: "approval_resolved",
        })}\n`,
      );

      const approvalResolvedEvent = await collector.waitFor(
        (event) => event.event === "reply_lifecycle" && event.phase === "approval_resolved",
        "expected approval_resolved lifecycle event from custom lifecycle bridge",
      );
      expect(approvalResolvedEvent).toMatchObject({
        event: "reply_lifecycle",
        phase: "approval_resolved",
        logicalReplyId: "reply_custom_1",
      });

      child.stdin.write(
        `${JSON.stringify({
          requestId: "lifecycle-6",
          command: "lifecycle_event",
          accountId: "default",
          logicalReplyId: "reply_custom_1",
          threadId: "oc_chat_custom_1",
          messageId: "om_custom_1",
          phase: "interrupt_requested",
        })}\n`,
      );

      const interruptRequestedEvent = await collector.waitFor(
        (event) => event.event === "reply_lifecycle" && event.phase === "interrupt_requested",
        "expected interrupt_requested lifecycle event from custom lifecycle bridge",
      );
      expect(interruptRequestedEvent).toMatchObject({
        event: "reply_lifecycle",
        phase: "interrupt_requested",
        logicalReplyId: "reply_custom_1",
      });

      child.stdin.write(
        `${JSON.stringify({
          requestId: "lifecycle-7",
          command: "lifecycle_event",
          accountId: "default",
          logicalReplyId: "reply_custom_1",
          threadId: "oc_chat_custom_1",
          messageId: "om_custom_1",
          phase: "resumed",
        })}\n`,
      );

      const resumedEvent = await collector.waitFor(
        (event) => event.event === "reply_lifecycle" && event.phase === "resumed",
        "expected resumed lifecycle event from custom lifecycle bridge",
      );
      expect(resumedEvent).toMatchObject({
        event: "reply_lifecycle",
        phase: "resumed",
        logicalReplyId: "reply_custom_1",
      });

      child.kill();
      await once(child, "exit");
    } finally {
      collector.close();
      child.kill();
    }
  });

  it("supports CommonJS plugins that require openclaw plugin-sdk subpaths", async () => {
    const pluginRoot = await createTempPluginRoot();
    await fs.writeFile(
      path.join(pluginRoot, "package.json"),
      JSON.stringify({
        openclaw: {
          extensions: ["./index.js"],
        },
      }),
      "utf8",
    );
    await fs.writeFile(
      path.join(pluginRoot, "index.js"),
      [
        "\"use strict\";",
        "const { normalizeAccountId } = require('openclaw/plugin-sdk/account-id');",
        "const { PAIRING_APPROVED_MESSAGE } = require('openclaw/plugin-sdk/channel-status');",
        "module.exports = {",
        "  register(api) {",
        "    api.registerChannel({",
        "      plugin: {",
        "        id: 'feishu',",
        "        config: {",
        "          resolveAccount() {",
        "            return { accountId: normalizeAccountId('default'), enabled: true, configured: true };",
        "          },",
        "        },",
        "        outbound: {",
        "          async sendText({ to, text, accountId }) {",
        "            return { delivered: true, channel: 'feishu', accountId, target: to, text, chatId: `plugin:${to}`, messageId: 'plugin_cjs_message_1' };",
        "          },",
        "        },",
        "        gateway: {",
        "          async startAccount({ setStatus }) {",
        "            setStatus({ running: true, pairingMessage: PAIRING_APPROVED_MESSAGE });",
        "          },",
        "        },",
        "      },",
        "    });",
        "  },",
        "};",
      ].join("\n"),
      "utf8",
    );

    const child = spawn(
      process.execPath,
      [
        path.join("scripts", "run-feishu-host.mjs"),
        "--plugin-root",
        pluginRoot,
        "--fixture-name",
        "runtime-cjs-plugin-sdk-subpaths",
        "--account-id",
        "default",
      ],
      {
        cwd: pluginHostDir,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

    const collector = createEventCollector(child.stdout, child.stderr);
    try {
      const readyEvent = await collector.waitFor(
        (event) => event.event === "ready",
        "expected ready event from CommonJS plugin fixture",
      );
      expect(readyEvent).toMatchObject({
        event: "ready",
        accountId: "default",
      });

      const statusEvent = await collector.waitFor(
        (event) =>
          event.event === "status" &&
          typeof event.patch === "object" &&
          event.patch !== null &&
          (event.patch as Record<string, unknown>).running === true,
        "expected running status from CommonJS plugin fixture",
      );
      expect(statusEvent).toMatchObject({
        event: "status",
        patch: expect.objectContaining({
          running: true,
          pairingMessage: "Pairing approved. You can now message this bot directly.",
        }),
      });

      child.stdin.end();
      await collector.waitFor(
        (event) => event.event === "stopped",
        "expected stopped event after CommonJS fixture stdin close",
      );
      await once(child, "exit");
    } finally {
      collector.close();
      child.kill();
    }
  }, 20000);

  it("supports CommonJS plugins whose internal files use import.meta.url", async () => {
    const pluginRoot = await createTempPluginRoot();
    await fs.writeFile(
      path.join(pluginRoot, "package.json"),
      JSON.stringify({
        openclaw: {
          extensions: ["./index.js"],
        },
      }),
      "utf8",
    );
    await fs.writeFile(
      path.join(pluginRoot, "index.js"),
      [
        "\"use strict\";",
        "const { getPluginVersion } = require('./src/core/version.js');",
        "module.exports = {",
        "  register(api) {",
        "    api.registerChannel({",
        "      plugin: {",
        "        id: 'feishu',",
        "        config: {",
        "          resolveAccount() {",
        "            return { accountId: 'default', enabled: true, configured: true };",
        "          },",
        "        },",
        "        outbound: {",
        "          async sendText({ to, text, accountId }) {",
        "            return { delivered: true, channel: 'feishu', accountId, target: to, text, chatId: `plugin:${to}`, messageId: 'plugin_cjs_import_meta_message_1' };",
        "          },",
        "        },",
        "        gateway: {",
        "          async startAccount({ setStatus }) {",
        "            setStatus({ running: true, version: getPluginVersion() });",
        "          },",
        "        },",
        "      },",
        "    });",
        "  },",
        "};",
      ].join("\n"),
      "utf8",
    );
    await fs.mkdir(path.join(pluginRoot, "src", "core"), { recursive: true });
    await fs.writeFile(
      path.join(pluginRoot, "src", "core", "version.js"),
      [
        "\"use strict\";",
        "Object.defineProperty(exports, \"__esModule\", { value: true });",
        "exports.getPluginVersion = getPluginVersion;",
        "const node_url_1 = require('node:url');",
        "const node_path_1 = require('node:path');",
        "const node_fs_1 = require('node:fs');",
        "let cachedVersion;",
        "function getPluginVersion() {",
        "  if (cachedVersion) return cachedVersion;",
        "  const __filename = (0, node_url_1.fileURLToPath)(import.meta.url);",
        "  const __dirname = (0, node_path_1.dirname)(__filename);",
        "  const pkg = JSON.parse((0, node_fs_1.readFileSync)((0, node_path_1.join)(__dirname, '..', '..', 'package.json'), 'utf8'));",
        "  cachedVersion = pkg.version ?? 'unknown';",
        "  return cachedVersion;",
        "}",
      ].join("\n"),
      "utf8",
    );

    const child = spawn(
      process.execPath,
      [
        path.join("scripts", "run-feishu-host.mjs"),
        "--plugin-root",
        pluginRoot,
        "--fixture-name",
        "runtime-cjs-import-meta",
        "--account-id",
        "default",
      ],
      {
        cwd: pluginHostDir,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

    const collector = createEventCollector(child.stdout, child.stderr);
    try {
      const readyEvent = await collector.waitFor(
        (event) => event.event === "ready",
        "expected ready event from CommonJS import.meta fixture",
      );
      expect(readyEvent).toMatchObject({
        event: "ready",
        accountId: "default",
      });

      const statusEvent = await collector.waitFor(
        (event) =>
          event.event === "status" &&
          typeof event.patch === "object" &&
          event.patch !== null &&
          (event.patch as Record<string, unknown>).running === true,
        "expected running status from CommonJS import.meta fixture",
      );
      expect(statusEvent).toMatchObject({
        event: "status",
        patch: expect.objectContaining({
          running: true,
          version: "unknown",
        }),
      });

      child.stdin.end();
      await collector.waitFor(
        (event) => event.event === "stopped",
        "expected stopped event after CommonJS import.meta fixture stdin close",
      );
      await once(child, "exit");
    } finally {
      collector.close();
      child.kill();
    }
  }, 20000);

  it("supports CommonJS plugins whose internal files use createRequire fallback with import.meta.url", async () => {
    const pluginRoot = await createTempPluginRoot();
    await fs.writeFile(
      path.join(pluginRoot, "package.json"),
      JSON.stringify({
        openclaw: {
          extensions: ["./index.js"],
        },
      }),
      "utf8",
    );
    await fs.writeFile(
      path.join(pluginRoot, "index.js"),
      [
        "\"use strict\";",
        "const { getMarker } = require('./src/core/token-store.js');",
        "module.exports = {",
        "  register(api) {",
        "    api.registerChannel({",
        "      plugin: {",
        "        id: 'feishu',",
        "        config: {",
        "          resolveAccount() {",
        "            return { accountId: 'default', enabled: true, configured: true };",
        "          },",
        "        },",
        "        outbound: {",
        "          async sendText({ to, text, accountId }) {",
        "            return { delivered: true, channel: 'feishu', accountId, target: to, text, chatId: `plugin:${to}`, messageId: 'plugin_cjs_create_require_message_1' };",
        "          },",
        "        },",
        "        gateway: {",
        "          async startAccount({ setStatus }) {",
        "            setStatus({ running: true, marker: getMarker() });",
        "          },",
        "        },",
        "      },",
        "    });",
        "  },",
        "};",
      ].join("\n"),
      "utf8",
    );
    await fs.mkdir(path.join(pluginRoot, "src", "core"), { recursive: true });
    await fs.writeFile(
      path.join(pluginRoot, "src", "core", "token-store.js"),
      [
        "\"use strict\";",
        "Object.defineProperty(exports, \"__esModule\", { value: true });",
        "exports.getMarker = getMarker;",
        "const { createRequire } = require('node:module');",
        "const logger = require('./lark-logger.js');",
        "const _require = createRequire(typeof __filename !== 'undefined' ? __filename : import.meta.url);",
        "function getMarker() {",
        "  return logger.marker ?? typeof _require;",
        "}",
      ].join("\n"),
      "utf8",
    );
    await fs.writeFile(
      path.join(pluginRoot, "src", "core", "lark-logger.js"),
      [
        "\"use strict\";",
        "Object.defineProperty(exports, \"__esModule\", { value: true });",
        "exports.marker = 'create-require-fallback';",
      ].join("\n"),
      "utf8",
    );

    const child = spawn(
      process.execPath,
      [
        path.join("scripts", "run-feishu-host.mjs"),
        "--plugin-root",
        pluginRoot,
        "--fixture-name",
        "runtime-cjs-create-require-import-meta",
        "--account-id",
        "default",
      ],
      {
        cwd: pluginHostDir,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

    const collector = createEventCollector(child.stdout, child.stderr);
    try {
      const readyEvent = await collector.waitFor(
        (event) => event.event === "ready",
        "expected ready event from CommonJS createRequire fallback fixture",
      );
      expect(readyEvent).toMatchObject({
        event: "ready",
        accountId: "default",
      });

      const statusEvent = await collector.waitFor(
        (event) =>
          event.event === "status" &&
          typeof event.patch === "object" &&
          event.patch !== null &&
          (event.patch as Record<string, unknown>).running === true,
        "expected running status from CommonJS createRequire fallback fixture",
      );
      expect(statusEvent).toMatchObject({
        event: "status",
        patch: expect.objectContaining({
          running: true,
          marker: "create-require-fallback",
        }),
      });

      child.stdin.end();
      await collector.waitFor(
        (event) => event.event === "stopped",
        "expected stopped event after CommonJS createRequire fallback fixture stdin close",
      );
      await once(child, "exit");
    } finally {
      collector.close();
      child.kill();
    }
  }, 20000);

  it("supports CommonJS plugins that expose the plugin via exports.default", async () => {
    const pluginRoot = await createTempPluginRoot();
    await fs.writeFile(
      path.join(pluginRoot, "package.json"),
      JSON.stringify({
        openclaw: {
          extensions: ["./index.js"],
        },
      }),
      "utf8",
    );
    await fs.writeFile(
      path.join(pluginRoot, "index.js"),
      [
        "\"use strict\";",
        "Object.defineProperty(exports, \"__esModule\", { value: true });",
        "const plugin = {",
        "  register(api) {",
        "    api.registerChannel({",
        "      plugin: {",
        "        id: 'feishu',",
        "        config: {",
        "          resolveAccount() {",
        "            return { accountId: 'default', enabled: true, configured: true };",
        "          },",
        "        },",
        "        outbound: {",
        "          async sendText({ to, text, accountId }) {",
        "            return { delivered: true, channel: 'feishu', accountId, target: to, text, chatId: `plugin:${to}`, messageId: 'plugin_cjs_default_message_1' };",
        "          },",
        "        },",
        "        gateway: {",
        "          async startAccount({ setStatus }) {",
        "            setStatus({ running: true, exportStyle: 'exports.default' });",
        "          },",
        "        },",
        "      },",
        "    });",
        "  },",
        "};",
        "exports.default = plugin;",
      ].join("\n"),
      "utf8",
    );

    const child = spawn(
      process.execPath,
      [
        path.join("scripts", "run-feishu-host.mjs"),
        "--plugin-root",
        pluginRoot,
        "--fixture-name",
        "runtime-cjs-exports-default",
        "--account-id",
        "default",
      ],
      {
        cwd: pluginHostDir,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

    const collector = createEventCollector(child.stdout, child.stderr);
    try {
      const readyEvent = await collector.waitFor(
        (event) => event.event === "ready",
        "expected ready event from CommonJS exports.default fixture",
      );
      expect(readyEvent).toMatchObject({
        event: "ready",
        accountId: "default",
      });

      const statusEvent = await collector.waitFor(
        (event) =>
          event.event === "status" &&
          typeof event.patch === "object" &&
          event.patch !== null &&
          (event.patch as Record<string, unknown>).running === true,
        "expected running status from CommonJS exports.default fixture",
      );
      expect(statusEvent).toMatchObject({
        event: "status",
        patch: expect.objectContaining({
          running: true,
          exportStyle: "exports.default",
        }),
      });

      child.stdin.end();
      await collector.waitFor(
        (event) => event.event === "stopped",
        "expected stopped event after CommonJS exports.default fixture stdin close",
      );
      await once(child, "exit");
    } finally {
      collector.close();
      child.kill();
    }
  }, 20000);

  it("captures plugin console output as structured log events instead of invalid protocol lines", async () => {
    const pluginRoot = await createTempPluginRoot();
    await fs.writeFile(
      path.join(pluginRoot, "package.json"),
      JSON.stringify({
        type: "module",
        openclaw: {
          extensions: ["./index.js"],
        },
      }),
      "utf8",
    );
    await fs.writeFile(
      path.join(pluginRoot, "index.js"),
      [
        "export default {",
        "  register(api) {",
        "    api.registerChannel({",
        "      plugin: {",
        "        id: 'feishu',",
        "        config: {",
        "          resolveAccount() {",
        "            return { accountId: 'default', enabled: true, configured: true };",
        "          },",
        "        },",
        "        outbound: {",
        "          async sendText({ to, text, accountId }) {",
        "            return { delivered: true, channel: 'feishu', accountId, target: to, text, chatId: `plugin:${to}`, messageId: 'plugin_console_message_1' };",
        "          },",
        "        },",
        "        gateway: {",
        "          async startAccount({ setStatus, abortSignal }) {",
        "            console.log('        Receive events/callbacks through persistent connection(使用 长连接 接收事件/回调)');",
        "            console.info(['[ws]', 'ws client ready']);",
        "            setStatus({ running: true });",
        "            await new Promise((resolve) => abortSignal.addEventListener('abort', resolve, { once: true }));",
        "          },",
        "        },",
        "      },",
        "    });",
        "  },",
        "};",
      ].join("\n"),
      "utf8",
    );

    const child = spawn(
      process.execPath,
      [
        path.join("scripts", "run-feishu-host.mjs"),
        "--plugin-root",
        pluginRoot,
        "--fixture-name",
        "runtime-console-bridge",
        "--account-id",
        "default",
      ],
      {
        cwd: pluginHostDir,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

    const collector = createEventCollector(child.stdout, child.stderr);
    try {
      await collector.waitFor(
        (event) => event.event === "ready",
        "expected ready event from console bridge fixture",
      );

      const runningStatus = await collector.waitFor(
        (event) =>
          event.event === "status" &&
          typeof event.patch === "object" &&
          event.patch !== null &&
          (event.patch as Record<string, unknown>).running === true,
        "expected running status from console bridge fixture",
      );
      expect(runningStatus).toMatchObject({
        event: "status",
        patch: expect.objectContaining({
          running: true,
        }),
      });

      const bannerLog = await collector.waitFor(
        (event) =>
          event.event === "log" &&
          event.scope === "console" &&
          String(event.message ?? "").includes("Receive events/callbacks through persistent connection"),
        "expected structured log event for console banner output",
      );
      expect(bannerLog).toMatchObject({
        event: "log",
        level: "info",
        scope: "console",
      });

      const wsLog = await collector.waitFor(
        (event) =>
          event.event === "log" &&
          event.scope === "console" &&
          String(event.message ?? "").includes("[ws]"),
        "expected structured log event for console array output",
      );
      expect(wsLog).toMatchObject({
        event: "log",
        level: "info",
        scope: "console",
      });

      expect(collector.events.some((event) => event.event === "fatal")).toBe(false);

      child.kill();
      await once(child, "exit");
    } finally {
      collector.close();
      child.kill();
    }
  }, 20000);
});
