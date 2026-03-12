import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { createSidecarApp } from '../src/index.js';

test('browser bridge endpoint accepts credentials report envelopes', async () => {
  const app = createSidecarApp();

  const res = await app.fetch(
    new Request('http://localhost/api/browser-bridge/native-message', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        version: 1,
        sessionId: 'sess-bridge-1',
        kind: 'request',
        payload: {
          type: 'credentials.report',
          appId: 'cli_123',
          appSecret: 'sec_456',
        },
      }),
    }),
  );

  assert.equal(res.status, 200);
  const json = await res.json();
  assert.deepEqual(json, {
    version: 1,
    sessionId: 'sess-bridge-1',
    kind: 'response',
    payload: {
      type: 'action.pause',
      reason: 'browser bridge credentials received',
      step: 'ENABLE_LONG_CONNECTION',
      title: '本地绑定已完成',
      instruction: '请前往事件与回调，开启长连接接受事件。',
      ctaLabel: '继续到事件与回调',
    },
  });
});

test('browser bridge endpoint rejects invalid envelopes', async () => {
  const app = createSidecarApp();

  const res = await app.fetch(
    new Request('http://localhost/api/browser-bridge/native-message', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        sessionId: 'sess-bridge-2',
      }),
    }),
  );

  assert.equal(res.status, 400);
  const json = await res.json();
  assert.equal(typeof json.error, 'string');
});

test('browser bridge endpoint forwards credentials to callback url when configured', async () => {
  const original = process.env.WORKCLAW_BROWSER_BRIDGE_CALLBACK_URL;
  const callbackRequests: unknown[] = [];
  const server = http.createServer(async (req, res) => {
    const chunks: Buffer[] = [];
    for await (const chunk of req) {
      chunks.push(Buffer.from(chunk));
    }
    callbackRequests.push(JSON.parse(Buffer.concat(chunks).toString('utf8')));
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(
      JSON.stringify({
        version: 1,
        sessionId: 'sess-bridge-3',
        kind: 'response',
        payload: {
          type: 'action.pause',
          reason: 'forwarded-to-callback',
          step: 'ENABLE_LONG_CONNECTION',
          title: '本地绑定已完成',
          instruction: '请前往事件与回调，开启长连接接受事件。',
          ctaLabel: '继续到事件与回调',
        },
      }),
    );
  });

  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', () => resolve()));
  const address = server.address();
  const port = typeof address === 'object' && address ? address.port : 0;
  process.env.WORKCLAW_BROWSER_BRIDGE_CALLBACK_URL = `http://127.0.0.1:${port}/browser-bridge/callback`;

  try {
    const app = createSidecarApp();
    const res = await app.fetch(
      new Request('http://localhost/api/browser-bridge/native-message', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          version: 1,
          sessionId: 'sess-bridge-3',
          kind: 'request',
          payload: {
            type: 'credentials.report',
            appId: 'cli_forward_123',
            appSecret: 'sec_forward_456',
          },
        }),
      }),
    );

    assert.equal(res.status, 200);
    const json = await res.json();
    assert.equal(callbackRequests.length, 1);
    assert.deepEqual(callbackRequests[0], {
      version: 1,
      sessionId: 'sess-bridge-3',
      kind: 'request',
      payload: {
        type: 'credentials.report',
        appId: 'cli_forward_123',
        appSecret: 'sec_forward_456',
      },
    });
    assert.deepEqual(json, {
      version: 1,
      sessionId: 'sess-bridge-3',
      kind: 'response',
      payload: {
        type: 'action.pause',
        reason: 'forwarded-to-callback',
        step: 'ENABLE_LONG_CONNECTION',
        title: '本地绑定已完成',
        instruction: '请前往事件与回调，开启长连接接受事件。',
        ctaLabel: '继续到事件与回调',
      },
    });
  } finally {
    process.env.WORKCLAW_BROWSER_BRIDGE_CALLBACK_URL = original;
    await new Promise<void>((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())));
  }
});
