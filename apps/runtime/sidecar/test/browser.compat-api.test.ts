import test from 'node:test';
import assert from 'node:assert/strict';
import { createSidecarApp } from '../src/index.js';

class FakeCompatBrowser {
  public calls: unknown[] = [];

  async compat(body: unknown) {
    this.calls.push(body);
    return {
      action: body && typeof body === 'object' && 'action' in body ? (body as Record<string, unknown>).action : null,
      profile: body && typeof body === 'object' && 'profile' in body ? (body as Record<string, unknown>).profile : null,
      running: true,
    };
  }
}

test('browser compat endpoint forwards action payloads', async () => {
  const browser = new FakeCompatBrowser();
  const app = createSidecarApp({ browser: browser as never });

  const res = await app.fetch(
    new Request('http://localhost/api/browser/compat', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ action: 'start', profile: 'openclaw' }),
    }),
  );

  assert.equal(res.status, 200);
  const json = await res.json();
  const payload = JSON.parse(String(json.output || 'null'));

  assert.equal(payload.profile, 'openclaw');
  assert.equal(payload.running, true);
  assert.deepEqual(browser.calls[0], { action: 'start', profile: 'openclaw' });
});
