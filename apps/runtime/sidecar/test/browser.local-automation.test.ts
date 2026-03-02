import test from 'node:test';
import assert from 'node:assert/strict';
import { BrowserController } from '../src/browser.js';

function buildLocalController() {
  const controller = new BrowserController() as any;
  controller.browser = {};
  controller.context = {};
  controller.page = {
    evaluate: async (arg1: unknown, arg2?: unknown) => {
      if (typeof arg1 === 'function') {
        return {
          url: 'https://example.com/',
          title: 'Example',
          items: [
            { tag: 'button', text: '提交', selector: '#submit' },
            { tag: 'input', text: '', selector: '#email' },
          ],
        };
      }
      return { ok: true, arg1, arg2 };
    },
    click: async (_selector: string) => undefined,
    fill: async (_selector: string, _text: string) => undefined,
    type: async (_selector: string, _text: string, _opts?: unknown) => undefined,
    hover: async (_selector: string) => undefined,
    selectOption: async (_selector: string, _values: string[]) => undefined,
    waitForTimeout: async (_ms: number) => undefined,
    waitForFunction: async (_fn: unknown, _arg?: unknown, _opts?: unknown) => undefined,
    keyboard: { press: async (_key: string) => undefined },
    dragAndDrop: async (_start: string, _end: string) => undefined,
    setViewportSize: async (_size: { width: number; height: number }) => undefined,
    url: () => 'https://example.com/',
    title: async () => 'Example',
  };
  return controller as BrowserController & { [k: string]: any };
}

test('snapshot builds local refs and text output', async () => {
  const controller = buildLocalController();
  const output = await controller.snapshot({ format: 'ai', interactive: true });
  const payload = JSON.parse(output);

  assert.equal(payload.format, 'ai');
  assert.equal(payload.url, 'https://example.com/');
  assert.equal(payload.stats.refs, 2);
  assert.equal(payload.refs.e1, '#submit');
  assert.match(payload.snapshot, /\[e1\]/);
});

test('act resolves ref to selector for click', async () => {
  const controller = buildLocalController();
  await controller.snapshot({ format: 'ai', interactive: true });

  let clicked = '';
  controller.page.click = async (selector: string) => {
    clicked = selector;
  };

  const output = await controller.act({ kind: 'click', ref: 'e1' });
  assert.equal(clicked, '#submit');
  assert.match(output, /已点击/);
});

test('act type with submit presses Enter', async () => {
  const controller = buildLocalController();
  await controller.snapshot({ format: 'ai', interactive: true });

  let filled = '';
  let pressed = '';
  controller.page.fill = async (_selector: string, text: string) => {
    filled = text;
  };
  controller.page.keyboard.press = async (key: string) => {
    pressed = key;
  };

  await controller.act({ kind: 'type', ref: 'e2', text: 'a@b.com', submit: true });
  assert.equal(filled, 'a@b.com');
  assert.equal(pressed, 'Enter');
});

test('act throws on unknown ref', async () => {
  const controller = buildLocalController();
  await assert.rejects(
    async () => controller.act({ kind: 'click', ref: 'e999' }),
    /selector 或 ref/,
  );
});
