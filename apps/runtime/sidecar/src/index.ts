import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { serve } from '@hono/node-server';
import { pathToFileURL } from 'node:url';
import { BrowserController } from './browser.js';
import { MCPManager } from './mcp.js';
import { FeishuClient } from './feishu.js';
import { FeishuLongConnectionManager } from './feishu_ws.js';
import { resolveRoute } from './openclaw-bridge/route-engine.js';
import { FeishuChannelAdapter } from './adapters/feishu/index.js';
import { WecomChannelAdapter } from './adapters/wecom/index.js';
import { ChannelAdapterKernel, createChannelAdapterRegistry } from './adapters/kernel.js';
import type { ApiResponse } from './types.js';

type RouteResolver = typeof resolveRoute;
type BrowserBridgeEnvelope = {
  version: 1;
  sessionId: string;
  kind: 'request' | 'response' | 'event';
  payload: {
    type: string;
    appId?: string;
    appSecret?: string;
    provider?: string;
    sessionId?: string;
    page?: string;
  };
};
type BrowserBridgeResponse = BrowserBridgeEnvelope;

interface SidecarDeps {
  browser: BrowserController;
  mcp: MCPManager;
  feishu: FeishuClient;
  feishuWs: FeishuLongConnectionManager;
  routeResolver: RouteResolver;
  browserBridge: (envelope: BrowserBridgeEnvelope) => Promise<BrowserBridgeResponse>;
  channelKernel: Pick<
    ChannelAdapterKernel,
    'start' | 'stop' | 'health' | 'drainEvents' | 'sendMessage' | 'catalog' | 'diagnostics' | 'ack' | 'replayEvents'
  >;
}

function normalizeConnectorId(input: unknown): string {
  const value = String(input || '').trim();
  return value || 'default';
}

function feishuInstanceId(employeeId: string): string {
  return `feishu:${normalizeConnectorId(employeeId)}`;
}

function healthToLegacyStatus(health: {
  state: string;
  last_ok_at: string | null;
  last_error: string | null;
  reconnect_attempts: number;
  queue_depth: number;
}) {
  return {
    running: health.state === 'running' || health.state === 'starting' || health.state === 'degraded',
    started_at: health.last_ok_at,
    queued_events: health.queue_depth,
    last_event_at: health.last_ok_at,
    last_error: health.last_error,
    reconnect_attempts: health.reconnect_attempts,
  };
}

function createDefaultDeps(): SidecarDeps {
  const browser = new BrowserController();
  const mcp = new MCPManager();
  const feishu = new FeishuClient();
  const feishuWs = new FeishuLongConnectionManager();
  const registry = createChannelAdapterRegistry();
  registry.register('feishu', new FeishuChannelAdapter(feishu, feishuWs));
  registry.register('wecom', new WecomChannelAdapter());
  const channelKernel = new ChannelAdapterKernel(registry);
  return {
    browser,
    mcp,
    feishu,
    feishuWs,
    routeResolver: resolveRoute,
    browserBridge: defaultBrowserBridgeHandler,
    channelKernel,
  };
}

async function defaultBrowserBridgeHandler(
  envelope: BrowserBridgeEnvelope,
): Promise<BrowserBridgeResponse> {
  const callbackUrl = String(process.env.WORKCLAW_BROWSER_BRIDGE_CALLBACK_URL || '').trim();
  if (callbackUrl) {
    const response = await fetch(callbackUrl, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
      },
      body: JSON.stringify(envelope),
    });
    const payload = await response.json();
    return payload as BrowserBridgeResponse;
  }

  if (envelope.payload?.type === 'credentials.report') {
    return {
      version: 1,
      sessionId: envelope.sessionId,
      kind: 'response',
      payload: {
        type: 'action.pause',
        reason: 'browser bridge credentials received',
        step: 'ENABLE_LONG_CONNECTION',
        title: '本地绑定已完成',
        instruction: '请前往事件与回调，开启长连接接受事件。',
        ctaLabel: '继续到事件与回调',
      } as BrowserBridgeResponse['payload'] & { reason: string },
    };
  }

  if (envelope.payload?.type === 'bridge.hello') {
    return {
      version: 1,
      sessionId: envelope.sessionId,
      kind: 'response',
      payload: {
        type: 'action.detect_step',
      },
    };
  }

  return {
    version: 1,
    sessionId: envelope.sessionId,
    kind: 'response',
    payload: {
      type: 'action.detect_step',
    },
  };
}

function isBrowserBridgeEnvelope(value: unknown): value is BrowserBridgeEnvelope {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const candidate = value as Partial<BrowserBridgeEnvelope>;
  return (
    candidate.version === 1 &&
    typeof candidate.sessionId === 'string' &&
    (candidate.kind === 'request' || candidate.kind === 'response' || candidate.kind === 'event') &&
    !!candidate.payload &&
    typeof candidate.payload === 'object' &&
    typeof (candidate.payload as { type?: unknown }).type === 'string'
  );
}

export function createSidecarApp(overrides: Partial<SidecarDeps> = {}) {
  const deps = { ...createDefaultDeps(), ...overrides } satisfies SidecarDeps;
  const app = new Hono();

  app.use('/*', cors());

  app.get('/health', (c) => {
    return c.json({ status: 'ok', uptime: process.uptime() });
  });

  app.post('/api/browser/launch', async (c) => {
    try {
      const body = await c.req.json();
      const result = await deps.browser.launch(body);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/navigate', async (c) => {
    try {
      const { url } = await c.req.json();
      const result = await deps.browser.navigate(url);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/click', async (c) => {
    try {
      const { selector } = await c.req.json();
      const result = await deps.browser.click(selector);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/type', async (c) => {
    try {
      const { selector, text, delay } = await c.req.json();
      const result = await deps.browser.type(selector, text, delay);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/scroll', async (c) => {
    try {
      const { direction, amount } = await c.req.json();
      const result = await deps.browser.scroll(direction, amount);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/hover', async (c) => {
    try {
      const { selector } = await c.req.json();
      const result = await deps.browser.hover(selector);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/press_key', async (c) => {
    try {
      const { key, modifiers } = await c.req.json();
      const result = await deps.browser.pressKey(key, modifiers);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/get_dom', async (c) => {
    try {
      const { selector, max_depth } = await c.req.json();
      const result = await deps.browser.getDOM(selector, max_depth);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/wait_for', async (c) => {
    try {
      const { selector, condition, timeout } = await c.req.json();
      const result = await deps.browser.waitFor({ selector, condition, timeout });
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/go_back', async (c) => {
    try {
      const result = await deps.browser.goBack();
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/go_forward', async (c) => {
    try {
      const result = await deps.browser.goForward();
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/reload', async (c) => {
    try {
      const result = await deps.browser.reload();
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/get_state', async (c) => {
    try {
      const result = await deps.browser.getState();
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/snapshot', async (c) => {
    try {
      const body = await c.req.json().catch(() => ({}));
      const result = await deps.browser.snapshot(body);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/act', async (c) => {
    try {
      const body = await c.req.json();
      const result = await deps.browser.act(body);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/screenshot', async (c) => {
    try {
      const { path } = await c.req.json();
      const result = await deps.browser.screenshot(path);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/evaluate', async (c) => {
    try {
      const { script } = await c.req.json();
      const result = await deps.browser.evaluate(script);
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/content', async (c) => {
    try {
      const result = await deps.browser.getContent();
      return c.json({ output: result } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser/close', async (c) => {
    try {
      await deps.browser.close();
      return c.json({ output: '浏览器已关闭' } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/mcp/add-server', async (c) => {
    try {
      const { name, command, args, env } = await c.req.json();
      await deps.mcp.addServer(name, { command, args, env });
      return c.json({ output: `MCP 服务器 ${name} 已添加` } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/mcp/list-servers', async (c) => {
    try {
      const servers = deps.mcp.listServers();
      return c.json({ output: JSON.stringify(servers) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/mcp/call-tool', async (c) => {
    try {
      const { server_name, tool_name, arguments: args } = await c.req.json();
      const result = await deps.mcp.callTool(server_name, tool_name, args);
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/mcp/list-tools', async (c) => {
    try {
      const { server_name } = await c.req.json();
      const tools = await deps.mcp.listTools(server_name);
      return c.json({ output: JSON.stringify(tools) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/web/search', async (c) => {
    try {
      const { query, count = 5 } = await c.req.json();
      const url = `https://html.duckduckgo.com/html/?q=${encodeURIComponent(query)}`;
      const resp = await fetch(url, {
        headers: { 'User-Agent': 'WorkClaw/1.0' },
      });
      const html = await resp.text();

      const results: { title: string; url: string; snippet: string }[] = [];
      const resultRegex = /<a[^>]*class="result__a"[^>]*href="([^"]*)"[^>]*>(.*?)<\/a>[\s\S]*?<a[^>]*class="result__snippet"[^>]*>(.*?)<\/a>/gi;
      let match;
      while ((match = resultRegex.exec(html)) !== null && results.length < count) {
        results.push({
          url: match[1].replace(/\/\/duckduckgo\.com\/l\/\?uddg=/, '').split('&')[0],
          title: match[2].replace(/<[^>]+>/g, '').trim(),
          snippet: match[3].replace(/<[^>]+>/g, '').trim(),
        });
      }

      if (results.length === 0) {
        const linkRegex = /<a[^>]*class="result__a"[^>]*href="([^"]*)"[^>]*>([\s\S]*?)<\/a>/gi;
        while ((match = linkRegex.exec(html)) !== null && results.length < count) {
          const rawUrl = match[1];
          const decodedUrl = rawUrl.includes('uddg=')
            ? decodeURIComponent(rawUrl.split('uddg=')[1]?.split('&')[0] || rawUrl)
            : rawUrl;
          results.push({
            url: decodedUrl,
            title: match[2].replace(/<[^>]+>/g, '').trim(),
            snippet: '',
          });
        }
      }

      const output = results
        .map((r, i) => `${i + 1}. ${r.title}\n   ${r.url}\n   ${r.snippet}`)
        .join('\n\n');

      return c.json({ output: output || '未找到搜索结果' } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/browser-bridge/native-message', async (c) => {
    try {
      const body = await c.req.json();
      if (!isBrowserBridgeEnvelope(body)) {
        return c.json({ error: 'invalid browser bridge envelope' }, 400);
      }
      const response = await deps.browserBridge(body);
      return c.json(response);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/channels/start', async (c) => {
    try {
      const body = await c.req.json();
      const result = await deps.channelKernel.start({
        adapter_name: String(body?.adapter_name || ''),
        connector_id: normalizeConnectorId(body?.connector_id),
        settings: body?.settings && typeof body.settings === 'object' ? body.settings : {},
      });
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/channels/stop', async (c) => {
    try {
      const body = await c.req.json().catch(() => ({}));
      await deps.channelKernel.stop(String(body?.instance_id || ''));
      return c.json({ output: JSON.stringify({ ok: true }) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/channels/health', async (c) => {
    try {
      const body = await c.req.json();
      const result = await deps.channelKernel.health(String(body?.instance_id || ''));
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/channels/drain-events', async (c) => {
    try {
      const body = await c.req.json().catch(() => ({}));
      const result = await deps.channelKernel.drainEvents(
        String(body?.instance_id || ''),
        Number(body?.limit || 50),
      );
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/channels/send-message', async (c) => {
    try {
      const body = await c.req.json();
      const result = await deps.channelKernel.sendMessage(
        String(body?.instance_id || ''),
        body?.request || {},
      );
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/channels/catalog', async (c) => {
    try {
      await c.req.json().catch(() => ({}));
      const result = await deps.channelKernel.catalog();
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/channels/diagnostics', async (c) => {
    try {
      const body = await c.req.json().catch(() => ({}));
      const result = await deps.channelKernel.diagnostics(String(body?.instance_id || ''));
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/channels/ack', async (c) => {
    try {
      const body = await c.req.json().catch(() => ({}));
      await deps.channelKernel.ack(String(body?.instance_id || ''), {
        message_id: String(body?.message_id || ''),
        status: body?.status ? String(body.status) : undefined,
      });
      return c.json({ output: JSON.stringify({ ok: true }) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/channels/replay-events', async (c) => {
    try {
      const body = await c.req.json().catch(() => ({}));
      const result = await deps.channelKernel.replayEvents(
        String(body?.instance_id || ''),
        Number(body?.limit || 50),
      );
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/openclaw/resolve-route', async (c) => {
    try {
      const body = await c.req.json();
      const result = deps.routeResolver({
        channel: String(body?.channel || ''),
        accountId: body?.account_id ?? undefined,
        peer: body?.peer ?? null,
        parentPeer: body?.parent_peer ?? null,
        guildId: body?.guild_id ?? undefined,
        teamId: body?.team_id ?? undefined,
        memberRoleIds: Array.isArray(body?.member_role_ids) ? body.member_role_ids : [],
        bindings: Array.isArray(body?.bindings) ? body.bindings : [],
        defaultAgentId: String(body?.default_agent_id || 'main'),
      });
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 400);
    }
  });

  app.post('/api/feishu/send-message', async (c) => {
    try {
      const body = await c.req.json();
      const employeeId = normalizeConnectorId(body?.employee_id);
      const instanceId = feishuInstanceId(employeeId);
      const result = await deps.channelKernel.sendMessage(instanceId, {
        channel: 'feishu',
        thread_id: String(body?.receive_id || ''),
        reply_target: null,
        text: typeof body?.content === 'string' ? body.content : '',
      });
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      const body = await c.req.json().catch(() => ({}));
      try {
        const result = await deps.feishu.sendMessage(body);
        return c.json({ output: JSON.stringify(result) } as ApiResponse);
      } catch (fallbackError: any) {
        return c.json({ error: fallbackError.message || e.message } as ApiResponse, 500);
      }
    }
  });

  app.post('/api/feishu/list-chats', async (c) => {
    try {
      const body = await c.req.json().catch(() => ({}));
      const result = await deps.feishu.listChats(body || {});
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/feishu/ws/start', async (c) => {
    try {
      const body = await c.req.json();
      const employeeId = normalizeConnectorId(body?.employee_id);
      const instanceId = feishuInstanceId(employeeId);
      await deps.channelKernel.start({
        adapter_name: 'feishu',
        connector_id: employeeId,
        settings: {
          ...body,
          employee_id: employeeId,
        },
      });
      const health = await deps.channelKernel.health(instanceId);
      return c.json({
        output: JSON.stringify({
          employee_id: employeeId,
          ...healthToLegacyStatus(health),
        }),
      } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/feishu/ws/stop', async (_c) => {
    try {
      const body = await _c.req.json().catch(() => ({}));
      const employeeId = normalizeConnectorId(body?.employee_id);
      await deps.channelKernel.stop(feishuInstanceId(employeeId));
      return _c.json({
        output: JSON.stringify({
          employee_id: employeeId,
          running: false,
          started_at: null,
          queued_events: 0,
          last_event_at: null,
          last_error: null,
          reconnect_attempts: 0,
        }),
      } as ApiResponse);
    } catch (e: any) {
      return _c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/feishu/ws/status', async (_c) => {
    try {
      const body = await _c.req.json().catch(() => ({}));
      if (body?.employee_id) {
        const employeeId = normalizeConnectorId(body.employee_id);
        const health = await deps.channelKernel.health(feishuInstanceId(employeeId));
        return _c.json({
          output: JSON.stringify({
            employee_id: employeeId,
            ...healthToLegacyStatus(health),
          }),
        } as ApiResponse);
      }
      const result = deps.feishuWs.statusAll();
      return _c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return _c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/feishu/ws/drain-events', async (c) => {
    try {
      const body = await c.req.json().catch(() => ({}));
      if (body?.employee_id) {
        const result = await deps.channelKernel.drainEvents(
          feishuInstanceId(normalizeConnectorId(body.employee_id)),
          Number(body?.limit || 50),
        );
        return c.json({ output: JSON.stringify(result) } as ApiResponse);
      }
      const result = deps.feishuWs.drainAll(Number(body?.limit || 50), body?.employee_id);
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  app.post('/api/feishu/ws/reconcile', async (c) => {
    try {
      const body = await c.req.json().catch(() => ({}));
      const employees = Array.isArray(body?.employees) ? body.employees : [];
      const result = deps.feishuWs.reconcile(employees);
      return c.json({ output: JSON.stringify(result) } as ApiResponse);
    } catch (e: any) {
      return c.json({ error: e.message } as ApiResponse, 500);
    }
  });

  return app;
}

const defaultDeps = createDefaultDeps();
const app = createSidecarApp(defaultDeps);

const PORT = Number(process.env.PORT || 8765);
let started = false;
let shutdownHookInstalled = false;

export function startSidecarServer(port = PORT) {
  if (started) {
    return;
  }
  started = true;
  console.log(`[sidecar] Starting on http://localhost:${port}`);

  if (!shutdownHookInstalled) {
    shutdownHookInstalled = true;
    process.on('SIGINT', async () => {
      await defaultDeps.browser.close();
      await defaultDeps.mcp.closeAll();
      process.exit(0);
    });
  }

  serve({ fetch: app.fetch, port });
}

const entryArg = process.argv[1];
if (entryArg && pathToFileURL(entryArg).href === import.meta.url) {
  startSidecarServer();
}

export default app;
