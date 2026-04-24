import test from "node:test";
import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { tmpdir } from "node:os";

test("sanitize-wecom-captures script converts recorded samples into sanitized fixtures", async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), "workclaw-wecom-sanitize-"));
  const inputDir = join(tempRoot, "captures");
  const outputDir = join(tempRoot, "fixtures");
  const capturePath = join(inputDir, "capture-1.json");

  try {
    await mkdir(inputDir, { recursive: true });
    await writeFile(
      capturePath,
      JSON.stringify(
        {
          captured_at: "2026-04-22T10:00:00Z",
          source: "wecom-adapter",
          inbound_event: {
            connector_id: "connector-prod",
            corp_id: "ww-real-corp",
            agent_id: "100042",
            event_type: "message.receive",
            conversation_type: "group",
            conversation_id: "room-prod-9",
            topic_id: "topic-prod-9",
            root_id: "root-prod-9",
            message_id: "msg-prod-9",
            sender_id: "zhangsan",
            sender_name: "张三",
            text: "真实线上消息",
            mentions: [{ type: "user", id: "lisi", name: "李四" }],
            occurred_at: "2026-04-22T09:59:58Z",
            reply_target: "room-prod-9",
            raw_payload: {
              threadId: "topic-prod-9",
              rootId: "root-prod-9",
            },
          },
          normalized_event: {
            channel: "wecom",
            workspace_id: "ww-real-corp",
            account_id: "100042",
            thread_id: "room-prod-9",
            chat_type: "group",
            topic_id: "topic-prod-9",
            root_id: "root-prod-9",
            message_id: "msg-prod-9",
            sender_id: "zhangsan",
            sender_name: "张三",
            text: "真实线上消息",
            mentions: [{ type: "user", id: "lisi", name: "李四" }],
            raw_event_type: "message.receive",
            occurred_at: "2026-04-22T09:59:58Z",
            reply_target: "room-prod-9",
            routing_context: {
              peer: { kind: "group", id: "room-prod-9" },
              topic: { kind: "topic", id: "topic-prod-9" },
              parent_peer: null,
              guild_id: null,
              team_id: null,
              member_role_ids: [],
              identity_links: [],
            },
          },
        },
        null,
        2,
      ),
      "utf8",
    );

    const result = spawnSync(
      process.execPath,
      ["scripts/sanitize-wecom-captures.mjs", inputDir, outputDir],
      {
        cwd: join(import.meta.dirname, ".."),
        encoding: "utf8",
      },
    );

    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /Wrote 2 sanitized fixture files/);

    const inbound = JSON.parse(
      await readFile(join(outputDir, "capture-1.inbound.json"), "utf8"),
    );
    const normalized = JSON.parse(
      await readFile(join(outputDir, "capture-1.normalized.json"), "utf8"),
    );

    assert.equal(inbound.corp_id, "corp-1");
    assert.equal(inbound.agent_id, "agent-1");
    assert.equal(inbound.conversation_id, "room-1");
    assert.equal(inbound.topic_id, "topic-1");
    assert.equal(inbound.root_id, "root-1");
    assert.equal(inbound.sender_name, "用户A");
    assert.equal(inbound.text, "请处理这条消息");
    assert.equal(inbound.mentions[0].id, "id-1");

    assert.equal(normalized.workspace_id, "corp-1");
    assert.equal(normalized.account_id, "agent-1");
    assert.equal(normalized.thread_id, "room-1");
    assert.equal(normalized.topic_id, "topic-1");
    assert.equal(normalized.root_id, "root-1");
    assert.equal(normalized.routing_context.peer.id, "room-1");
    assert.equal(normalized.routing_context.topic.id, "topic-1");
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});
