import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { serve } from '@hono/node-server';
import { BrowserController } from './browser.js';
import { MCPManager } from './mcp.js';
import { FeishuClient } from './feishu.js';
import { FeishuLongConnectionManager } from './feishu_ws.js';
import type { ApiResponse } from './types.js';

const app = new Hono();
const browser = new BrowserController();
const mcp = new MCPManager();
const feishu = new FeishuClient();
const feishuWs = new FeishuLongConnectionManager();

app.use('/*', cors());

app.get('/health', (c) => {
  return c.json({ status: 'ok', uptime: process.uptime() });
});

// ─── 浏览器自动化端点 ──────────────────────────────────────────────

// 启动浏览器（支持 headless 和 viewport 选项）
app.post('/api/browser/launch', async (c) => {
  try {
    const body = await c.req.json();
    const result = await browser.launch(body);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 导航到指定 URL
app.post('/api/browser/navigate', async (c) => {
  try {
    const { url } = await c.req.json();
    const result = await browser.navigate(url);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 点击元素
app.post('/api/browser/click', async (c) => {
  try {
    const { selector } = await c.req.json();
    const result = await browser.click(selector);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 在元素中输入文本
app.post('/api/browser/type', async (c) => {
  try {
    const { selector, text, delay } = await c.req.json();
    const result = await browser.type(selector, text, delay);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 滚动页面（up / down / to_top / to_bottom）
app.post('/api/browser/scroll', async (c) => {
  try {
    const { direction, amount } = await c.req.json();
    const result = await browser.scroll(direction, amount);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 悬停在元素上
app.post('/api/browser/hover', async (c) => {
  try {
    const { selector } = await c.req.json();
    const result = await browser.hover(selector);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 按下键盘按键（支持修饰键组合）
app.post('/api/browser/press_key', async (c) => {
  try {
    const { key, modifiers } = await c.req.json();
    const result = await browser.pressKey(key, modifiers);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 获取简化 DOM 结构
app.post('/api/browser/get_dom', async (c) => {
  try {
    const { selector, max_depth } = await c.req.json();
    const result = await browser.getDOM(selector, max_depth);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 等待条件满足（selector 或 JS 条件表达式）
app.post('/api/browser/wait_for', async (c) => {
  try {
    const { selector, condition, timeout } = await c.req.json();
    const result = await browser.waitFor({ selector, condition, timeout });
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 后退
app.post('/api/browser/go_back', async (c) => {
  try {
    const result = await browser.goBack();
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 前进
app.post('/api/browser/go_forward', async (c) => {
  try {
    const result = await browser.goForward();
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 刷新页面
app.post('/api/browser/reload', async (c) => {
  try {
    const result = await browser.reload();
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 获取浏览器状态（URL、标题、是否运行中）
app.post('/api/browser/get_state', async (c) => {
  try {
    const result = await browser.getState();
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 页面快照（本地生成，返回 ref -> selector 映射）
app.post('/api/browser/snapshot', async (c) => {
  try {
    const body = await c.req.json().catch(() => ({}));
    const result = await browser.snapshot(body);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 动作执行（支持基于 ref 的本地自动化）
app.post('/api/browser/act', async (c) => {
  try {
    const body = await c.req.json();
    const result = await browser.act(body);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 截取页面截图
app.post('/api/browser/screenshot', async (c) => {
  try {
    const { path } = await c.req.json();
    const result = await browser.screenshot(path);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 在页面中执行 JavaScript
app.post('/api/browser/evaluate', async (c) => {
  try {
    const { script } = await c.req.json();
    const result = await browser.evaluate(script);
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 获取页面 HTML 内容
app.post('/api/browser/content', async (c) => {
  try {
    const result = await browser.getContent();
    return c.json({ output: result } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// 关闭浏览器
app.post('/api/browser/close', async (c) => {
  try {
    await browser.close();
    return c.json({ output: '浏览器已关闭' } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// MCP endpoints
app.post('/api/mcp/add-server', async (c) => {
  try {
    const { name, command, args, env } = await c.req.json();
    await mcp.addServer(name, { command, args, env });
    return c.json({ output: `MCP 服务器 ${name} 已添加` } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/mcp/list-servers', async (c) => {
  try {
    const servers = mcp.listServers();
    return c.json({ output: JSON.stringify(servers) } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/mcp/call-tool', async (c) => {
  try {
    const { server_name, tool_name, arguments: args } = await c.req.json();
    const result = await mcp.callTool(server_name, tool_name, args);
    return c.json({ output: JSON.stringify(result) } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/mcp/list-tools', async (c) => {
  try {
    const { server_name } = await c.req.json();
    const tools = await mcp.listTools(server_name);
    return c.json({ output: JSON.stringify(tools) } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

// Web search endpoint (DuckDuckGo HTML)
app.post('/api/web/search', async (c) => {
  try {
    const { query, count = 5 } = await c.req.json();
    const url = `https://html.duckduckgo.com/html/?q=${encodeURIComponent(query)}`;
    const resp = await fetch(url, {
      headers: { 'User-Agent': 'SkillMint/1.0' },
    });
    const html = await resp.text();

    // 解析 DuckDuckGo HTML 结果
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

    // 如果正则没匹配到，尝试简单解析
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

// ─── Feishu (official SDK) ───────────────────────────────────────
app.post('/api/feishu/send-message', async (c) => {
  try {
    const body = await c.req.json();
    const result = await feishu.sendMessage(body);
    return c.json({ output: JSON.stringify(result) } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/feishu/list-chats', async (c) => {
  try {
    const body = await c.req.json().catch(() => ({}));
    const result = await feishu.listChats(body || {});
    return c.json({ output: JSON.stringify(result) } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/feishu/ws/start', async (c) => {
  try {
    const body = await c.req.json();
    const result = feishuWs.start(body || {});
    return c.json({ output: JSON.stringify(result) } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/feishu/ws/stop', async (_c) => {
  try {
    const result = feishuWs.stop();
    return _c.json({ output: JSON.stringify(result) } as ApiResponse);
  } catch (e: any) {
    return _c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/feishu/ws/status', async (_c) => {
  try {
    const result = feishuWs.status();
    return _c.json({ output: JSON.stringify(result) } as ApiResponse);
  } catch (e: any) {
    return _c.json({ error: e.message } as ApiResponse, 500);
  }
});

app.post('/api/feishu/ws/drain-events', async (c) => {
  try {
    const body = await c.req.json().catch(() => ({}));
    const result = feishuWs.drain(Number(body?.limit || 50));
    return c.json({ output: JSON.stringify(result) } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 500);
  }
});

const PORT = Number(process.env.PORT || 8765);
console.log(`[sidecar] Starting on http://localhost:${PORT}`);

// Graceful shutdown
process.on('SIGINT', async () => {
  await browser.close();
  await mcp.closeAll();
  process.exit(0);
});

serve({ fetch: app.fetch, port: PORT });

export default app;
