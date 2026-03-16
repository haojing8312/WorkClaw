import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { chromium } from 'playwright';
import { BrowserController } from '../src/browser.js';
import { mapCompatUploadPath } from '../src/browser_uploads.js';

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

function makeCompatPage(initialUrl = 'about:blank') {
  let currentUrl = initialUrl;
  let currentTitle = initialUrl === 'about:blank' ? 'Blank' : 'Example';

  return {
    goto: async (url: string) => {
      currentUrl = url;
      currentTitle = url.includes('example.com') ? 'Example' : 'Page';
    },
    evaluate: async (arg1: unknown, arg2?: unknown) => {
      if (typeof arg1 === 'function') {
        return {
          url: currentUrl,
          title: currentTitle,
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
    setInputFiles: async (_selector: string, _paths: string[]) => undefined,
    url: () => currentUrl,
    title: async () => currentTitle,
  };
}

function buildCompatController() {
  const controller = new BrowserController({ compatProfileRoot: 'E:/tmp/workclaw-browser-profiles' }) as any;
  controller.compatProfiles = new Map();
  controller.compatTargetIds = new WeakMap();
  controller.compatTargetSeq = 2;

  const page = makeCompatPage();
  const state = {
    profile: 'openclaw',
    context: {
      pages: () => [page],
      newPage: async () => page,
    },
    pages: new Map([['tab_1', page]]),
    activeTargetId: 'tab_1',
    refMaps: new Map(),
    userDataDir: 'E:/tmp/workclaw-browser-profiles/openclaw',
  };

  controller.compatProfiles.set('openclaw', state);
  controller.startCompatProfile = async () => state;

  return { controller, state, page };
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

test('compat start creates durable openclaw profile state', async () => {
  const controller = new BrowserController({ compatProfileRoot: 'E:/tmp/workclaw-browser-profiles' }) as any;
  controller.compatProfiles = new Map();

  let startedProfile = '';
  controller.startCompatProfile = async (profile: string) => {
    startedProfile = profile;
    const state = {
      profile,
      context: { pages: () => [] },
      pages: new Map(),
      activeTargetId: null,
      refMaps: new Map(),
      userDataDir: `E:/tmp/workclaw-browser-profiles/${profile}`,
    };
    controller.compatProfiles.set(profile, state);
    return state;
  };

  const result = await controller.compat({ action: 'start', profile: 'openclaw' });
  assert.equal(startedProfile, 'openclaw');
  assert.equal(result.profile, 'openclaw');
  assert.equal(result.running, true);
});

test('compat profiles lists only the supported openclaw profile in p0', async () => {
  const controller = new BrowserController({ compatProfileRoot: 'E:/tmp/workclaw-browser-profiles' }) as any;
  controller.compatProfiles = new Map();
  controller.startCompatProfile = async (profile: string) => {
    const state = {
      profile,
      context: { pages: () => [] },
      pages: new Map(),
      activeTargetId: null,
      refMaps: new Map(),
      userDataDir: `E:/tmp/workclaw-browser-profiles/${profile}`,
    };
    controller.compatProfiles.set(profile, state);
    return state;
  };

  await controller.compat({ action: 'start', profile: 'openclaw' });
  const result = await controller.compat({ action: 'profiles' });
  assert.match(JSON.stringify(result), /openclaw/);
  assert.doesNotMatch(JSON.stringify(result), /chrome/i);
});

test('compat stop closes the supported openclaw profile', async () => {
  const controller = new BrowserController({ compatProfileRoot: 'E:/tmp/workclaw-browser-profiles' }) as any;
  controller.compatProfiles = new Map();

  let closed = false;
  const state = {
    profile: 'openclaw',
    context: {
      pages: () => [],
      newPage: async () => makeCompatPage(),
      close: async () => {
        closed = true;
      },
    },
    pages: new Map(),
    activeTargetId: null,
    refMaps: new Map(),
    userDataDir: 'E:/tmp/workclaw-browser-profiles/openclaw',
  };

  controller.compatProfiles.set('openclaw', state);
  const result = await controller.compat({ action: 'stop', profile: 'openclaw' });

  assert.equal(closed, true);
  assert.equal(result.ok, true);
  assert.equal(controller.compatProfiles.has('openclaw'), false);
});

test('compat open returns a stable targetId and tabs exposes it', async () => {
  const { controller, page } = buildCompatController();

  const opened = await controller.compat({ action: 'open', profile: 'openclaw', url: 'https://example.com' });
  const tabs = await controller.compat({ action: 'tabs', profile: 'openclaw' });

  assert.equal(opened.targetId, 'tab_1');
  assert.equal(page.url(), 'https://example.com');
  assert.match(JSON.stringify(tabs), /tab_1/);
  assert.match(JSON.stringify(tabs), /example\.com/);
});

test('compat snapshot and act use the requested targetId', async () => {
  const { controller, state } = buildCompatController();
  const targetPage = makeCompatPage('https://target.example.com');
  let clicked = '';
  targetPage.click = async (selector: string) => {
    clicked = selector;
  };

  state.pages.set('tab_2', targetPage);
  state.activeTargetId = 'tab_1';

  const snap = await controller.compat({ action: 'snapshot', profile: 'openclaw', targetId: 'tab_2', interactive: true });
  assert.equal(snap.targetId, 'tab_2');

  await controller.compat({ action: 'act', profile: 'openclaw', targetId: 'tab_2', kind: 'click', selector: '#submit' });
  assert.equal(clicked, '#submit');
});

test('compat upload accepts a normal local file path and stages it', async () => {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'workclaw-upload-'));
  try {
    const localFile = path.join(tempRoot, 'cover.png');
    await writeFile(localFile, 'fake image payload');

    const { controller, state, page } = buildCompatController();
    controller.compatUploadRoot = path.join(tempRoot, 'staging');
    state.refMaps.set('tab_1', new Map([['e3', '#upload-input']]));

    let uploadedSelector = '';
    let uploadedPaths: string[] = [];
    page.setInputFiles = async (selector: string, paths: string[]) => {
      uploadedSelector = selector;
      uploadedPaths = paths;
    };

    const result = await controller.compat({
      action: 'upload',
      profile: 'openclaw',
      targetId: 'tab_1',
      inputRef: 'e3',
      paths: [localFile],
    });

    assert.equal(result.ok, true);
    assert.equal(uploadedSelector, '#upload-input');
    assert.equal(uploadedPaths.length, 1);
    assert.match(uploadedPaths[0], /staging/i);
    assert.notEqual(uploadedPaths[0], localFile);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test('mapCompatUploadPath maps /tmp/openclaw/uploads to a workclaw-owned staging path', async () => {
  const mapped = mapCompatUploadPath('/tmp/openclaw/uploads/cover.png', 'E:/tmp/workclaw-staging');
  assert.match(mapped, /openclaw[\\/]uploads/i);
  assert.match(mapped, /cover\.png$/i);
});

test('compat serializes concurrent openclaw profile startup', async () => {
  const controller = new BrowserController({ compatProfileRoot: 'E:/tmp/workclaw-browser-profiles' });
  const originalLaunchPersistentContext = chromium.launchPersistentContext.bind(chromium);

  let launches = 0;
  const page = makeCompatPage();
  const fakeContext = {
    addInitScript: async () => undefined,
    pages: () => [page],
    newPage: async () => page,
    browser: () => null,
    close: async () => undefined,
  };

  (chromium as typeof chromium & {
    launchPersistentContext: typeof chromium.launchPersistentContext;
  }).launchPersistentContext = (async () => {
    launches += 1;
    await new Promise((resolve) => setTimeout(resolve, 20));
    return fakeContext as never;
  }) as typeof chromium.launchPersistentContext;

  try {
    const [started, opened] = await Promise.all([
      controller.compat({ action: 'start', profile: 'openclaw' }),
      controller.compat({ action: 'open', profile: 'openclaw', url: 'https://example.com' }),
    ]);

    assert.equal(started.profile, 'openclaw');
    assert.equal(opened.targetId, 'tab_1');
    assert.equal(launches, 1);
  } finally {
    (chromium as typeof chromium & {
      launchPersistentContext: typeof chromium.launchPersistentContext;
    }).launchPersistentContext = originalLaunchPersistentContext;
    await controller.close();
  }
});
