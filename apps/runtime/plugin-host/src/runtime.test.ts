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

    await runtime.channel.reply.dispatchReplyFromConfig({
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

    await expect(result.dispatcher.waitForIdle()).resolves.toBeUndefined();
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
});
