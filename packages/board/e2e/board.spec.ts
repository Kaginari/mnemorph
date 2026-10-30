/**
 * The board end to end, at phone (390×844) and desktop (1440×900) in both themes: screenshots,
 * rendered text contrast (WCAG AA), a label on every coloured node, the router, the drawers, the
 * theme and hostile text.
 */
import { fileURLToPath } from 'node:url';
import type { BoardOverview } from '@mnemorph/protocol';
import { expect, type Page, test, type WebSocketRoute } from '@playwright/test';
import { FIXTURE_NOW, fixtureOverviews, fixtureWorlds } from '../src/data/fixtures.ts';

/** Where the screenshots go: test-results/screens/<project>-<theme>-<page>.png. */
const screen = (file: string) =>
  fileURLToPath(new URL(`../test-results/screens/${file}.png`, import.meta.url));

const LIVE = fixtureWorlds.worlds.find((w) => w.name === 'tempest-try');
if (!LIVE) throw new Error('the fixtures hold tempest-try');

/**
 * Opens the board at `hash`, with `/api/worlds` answering `body` and `/api/w/<id>` the overview
 * `overviews` holds (404 for another). The push socket is the test's: `sockets` collects each one
 * the page opens. Every CSP violation the page reports is collected, and `violations` reads them:
 * the page runs under the board server's own CSP.
 */
async function open(
  page: Page,
  hash: string,
  body: unknown = fixtureWorlds,
  overviews: Record<string, unknown> = fixtureOverviews,
  sockets: WebSocketRoute[] = [],
) {
  await page.addInitScript(() => {
    const seen: string[] = [];
    (window as { __csp?: string[] }).__csp = seen;
    document.addEventListener('securitypolicyviolation', (e) =>
      seen.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  await page.clock.setFixedTime(FIXTURE_NOW);
  await page.route('**/api/worlds', (r) =>
    r.fulfill({ contentType: 'application/json', body: JSON.stringify(body) }),
  );
  await page.route('**/api/session', (r) =>
    r.fulfill({ contentType: 'application/json', body: JSON.stringify({ csrf: 'ab'.repeat(32) }) }),
  );
  await page.route('**/api/w/*', (r) => {
    const id = decodeURIComponent(new URL(r.request().url()).pathname.split('/').pop() ?? '');
    const o = overviews[id];
    return o
      ? r.fulfill({ contentType: 'application/json', body: JSON.stringify(o) })
      : r.fulfill({ status: 404, contentType: 'text/plain', body: 'no listed world has this id' });
  });
  await page.routeWebSocket('**/ws/board', (ws) => {
    sockets.push(ws);
  });
  await page.goto(`/${hash}`);
  await expect(page.locator('.shell')).toBeVisible();
}

/** The CSP violations the page has reported. */
function violations(page: Page) {
  return page.evaluate(() => (window as { __csp?: string[] }).__csp ?? ['no CSP listener']);
}

/**
 * Every visible text node whose colour misses WCAG AA on what is behind it: 4.5:1, or 3:1 for
 * large text. Backgrounds are composited up the tree over the page's own.
 */
function lowContrast(page: Page) {
  return page.evaluate(() => {
    type C = [number, number, number, number];
    const parse = (s: string): C | undefined => {
      const n = (s.match(/-?[\d.]+(?:e-?\d+)?/g) ?? []).map(Number);
      const [r = 0, g = 0, b = 0, a = 1] = n;
      if (s === 'transparent') return [0, 0, 0, 0];
      if (s.startsWith('color')) return [r * 255, g * 255, b * 255, a];
      if (s.startsWith('rgb') && n.length >= 3) return [r, g, b, a];
      return undefined;
    };
    const over = (top: C, under: C): C => {
      const a = top[3] + under[3] * (1 - top[3]);
      if (a === 0) return [0, 0, 0, 0];
      const m = (i: 0 | 1 | 2) => (top[i] * top[3] + under[i] * under[3] * (1 - top[3])) / a;
      return [m(0), m(1), m(2), a];
    };
    const lum = (c: C) => {
      const ch = (v: number) => {
        const x = v / 255;
        return x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
      };
      return 0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2]);
    };
    const backdrop = (el: Element | null): C => {
      const layers: C[] = [];
      for (let e = el; e; e = e.parentElement) {
        const c = parse(getComputedStyle(e).backgroundColor);
        if (c && c[3] > 0) layers.push(c);
        if (c && c[3] >= 1) break;
      }
      let acc: C = [255, 255, 255, 1];
      for (const l of layers.reverse()) acc = over(l, acc);
      return acc;
    };
    const out: string[] = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      const el = n.parentElement;
      if (!el || !n.textContent?.trim()) continue;
      const r = el.getBoundingClientRect();
      const cs = getComputedStyle(el);
      if (r.width === 0 || r.height === 0 || cs.visibility === 'hidden') continue;
      if (el.closest('[hidden]')) continue;
      const fg = parse(cs.color);
      if (!fg) {
        out.push(`${n.textContent.trim()}: unreadable colour ${cs.color}`);
        continue;
      }
      if (fg[3] === 0) continue; // gradient text: the wordmark, a logotype
      const bg = backdrop(el);
      const shown = over(fg, bg);
      const [hi, lo] = [lum(shown), lum(bg)].sort((a, b) => b - a) as [number, number];
      const ratio = (hi + 0.05) / (lo + 0.05);
      const px = Number.parseFloat(cs.fontSize);
      const large = px >= 24 || (px >= 18.66 && Number(cs.fontWeight) >= 700);
      const disabled = el.closest(':disabled, [aria-disabled="true"]') !== null;
      if (!disabled && ratio < (large ? 3 : 4.5)) {
        out.push(`${n.textContent.trim().slice(0, 40)}: ${ratio.toFixed(2)}`);
      }
    }
    return out;
  });
}

/** Coloured nodes without words beside them. */
function unlabelled(page: Page) {
  return page.$$eval('[data-tone], [data-race]', (els) =>
    els
      .filter((e) => !(e.querySelector('.mark-label')?.textContent ?? '').trim())
      .map((e) => `${e.tagName} ${e.className}`),
  );
}

const byName = (name: string) => {
  const w = fixtureWorlds.worlds.find((x) => x.name === name);
  if (!w) throw new Error(`the fixtures hold ${name}`);
  return w;
};
const SQLGLOT = byName('sqlglot');
const NOTES = byName('notes');
const GONE = byName('old-spike');

const PAGES: [string, string][] = [
  ['home', '#/'],
  ['world-live', `#/w/${LIVE.id}`],
  ['world-tui-live', `#/w/${SQLGLOT.id}`],
  ['world-silent', `#/w/${NOTES.id}`],
  ['world-missing', `#/w/${GONE.id}`],
  ['session', `#/w/${LIVE.id}/s/r-17`],
  ['settings', '#/settings'],
];

for (const theme of ['dark', 'light'] as const) {
  test.describe(`${theme} theme`, () => {
    test.use({ colorScheme: theme });

    for (const [name, hash] of PAGES) {
      test(`${name}: screenshot, contrast, labels`, async ({ page }, info) => {
        await open(page, hash);
        await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
        await expect(page.locator('.shell[data-load="ready"]')).toBeAttached();
        await page.screenshot({
          path: screen(`${info.project.name}-${theme}-${name}`),
        });
        expect(await lowContrast(page)).toEqual([]);
        expect(await unlabelled(page)).toEqual([]);
        expect(await violations(page)).toEqual([]);
      });
    }

    test('the open drawer and pane read too', async ({ page }, info) => {
      await open(page, '#/');
      if (info.project.name === 'phone') {
        await page.getByRole('button', { name: 'Menu' }).click();
        await expect(page.locator('#rail')).toBeVisible();
        await page.screenshot({
          path: screen(`${info.project.name}-${theme}-rail`),
        });
      }
      expect(await lowContrast(page)).toEqual([]);
      expect(await unlabelled(page)).toEqual([]);
    });
  });
}

test("the page runs under the board server's CSP", async ({ page }) => {
  const res = await page.request.get('/');
  expect(res.headers()['content-security-policy']).toContain("require-trusted-types-for 'script'");
  await open(page, '#/settings');
  await page.getByRole('radio', { name: 'light' }).check();
  await page.goto(`/#/w/${LIVE.id}/s/r-17`);
  await expect(page.locator('.page-title')).toContainText('r-17');
  expect(await violations(page)).toEqual([]);
});

test('every world reads in words, the latest first', async ({ page }, info) => {
  await open(page, '#/');
  const cards = page.locator('.world-card');
  await expect(cards).toHaveCount(4);
  await expect(cards.first()).toContainText('tempest-try');
  await expect(cards.first()).toContainText('live');
  await expect(cards.last()).toContainText('never opened');
  await expect(page.locator('.world-card', { hasText: 'old-spike' })).toContainText('missing');
  if (info.project.name === 'desktop') {
    await expect(page.locator('.rail-world')).toHaveCount(4);
  }
});

test('deep links land, the back button returns', async ({ page }) => {
  await open(page, '#/');
  await page.goto(`/#/w/${LIVE.id}/memory`);
  await expect(page.locator('.page-title')).toContainText('Memory');
  await page.goto(`/#/w/${LIVE.id}/s/r-17`);
  await expect(page.locator('.page-title')).toContainText('r-17');
  await page.goBack();
  await expect(page.locator('.page-title')).toContainText('Memory');
  await expect(page).toHaveTitle(/tempest-try · Mnemorph/);
});

test('an unknown address and an unknown world are said to be', async ({ page }) => {
  await open(page, '#/nowhere/at/all');
  await expect(page.getByText('Nothing at this address')).toBeVisible();
  await page.goto('/#/w/ghost-0/s/x');
  await expect(page.getByText('No such world')).toBeVisible();
});

test('the theme switch holds across a reload', async ({ page }) => {
  await open(page, '#/settings');
  await page.getByRole('radio', { name: 'light' }).check();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.getByRole('radio', { name: 'dark' }).check();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('the rail is a drawer at phone width', async ({ page }, info) => {
  test.skip(info.project.name !== 'phone', 'phone only');
  await open(page, '#/');
  await expect(page.locator('#rail')).toBeHidden();
  expect(await page.locator('#rail').evaluate((e) => (e as HTMLElement).inert)).toBe(true);
  const menu = page.getByRole('button', { name: 'Menu' });
  await menu.click();
  await expect(page.locator('#rail')).toBeVisible();
  expect(await page.locator('#rail').evaluate((e) => (e as HTMLElement).inert)).toBe(false);
  await expect(page.locator('#rail :focus')).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(page.locator('#rail')).toBeHidden();
  await expect(menu).toBeFocused();
  await menu.click();
  await page.locator('.rail-world', { hasText: 'sqlglot' }).click();
  await expect(page.locator('#rail')).toBeHidden();
  await expect(page.locator('.page-title')).toContainText('sqlglot');
});

test('the side pane is a drawer at phone width, focus in and back', async ({ page }, info) => {
  test.skip(info.project.name !== 'phone', 'phone only');
  await open(page, `#/w/${LIVE.id}`);
  await expect(page.locator('#pane')).toBeHidden();
  const toggle = page.getByRole('button', { name: 'Show the side pane' });
  await toggle.click();
  await expect(page.locator('#pane')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('#pane')).toBeHidden();
  await expect(page.getByRole('button', { name: 'Show the side pane' })).toBeFocused();
});

test('the side pane folds, and stays folded', async ({ page }, info) => {
  test.skip(info.project.name !== 'desktop', 'desktop only');
  await open(page, '#/');
  await expect(page.locator('#pane')).toBeVisible();
  await page.getByRole('button', { name: 'Hide the side pane' }).click();
  await expect(page.locator('#pane')).toBeHidden();
  await page.reload();
  await expect(page.locator('#pane')).toBeHidden();
  await page.getByRole('button', { name: 'Show the side pane' }).click();
  await expect(page.locator('#pane')).toBeVisible();
});

test('hostile text from the server stays text', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const evil = {
    ...LIVE,
    id: 'evil-1',
    name: '<img src=x onerror="window.__pwned=1">\u202egnp.exe',
    path: '/tmp/<script>window.__pwned=2</script>',
    remote: 'javascript:window.__pwned=3',
  };
  await open(page, '#/w/evil-1', { worlds: [evil] });
  await expect(page.locator('.page-title')).toContainText('<img src=x');
  await expect(page.locator('.page-title .safe-esc')).toHaveText('\\u{202e}');
  expect(
    await page.locator('main img, main script, #pane a, main a[href^="javascript"]').count(),
  ).toBe(0);
  expect(await page.evaluate(() => (window as { __pwned?: number }).__pwned)).toBeUndefined();
  expect(errors).toEqual([]);
});

test('a page that loads with no console error', async ({ page }) => {
  const errors: string[] = [];
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  await open(page, '#/');
  await expect(page.locator('.world-card').first()).toBeVisible();
  expect(errors).toEqual([]);
});

test('each home row says its world: status, standing, gate, cost, what waits', async ({ page }) => {
  await open(page, '#/');
  const row = (w: { name: string }) => page.locator('.world-card', { hasText: w.name });
  await expect(row(LIVE)).toContainText('live');
  await expect(row(LIVE)).toContainText('grounded 41');
  await expect(row(LIVE)).toContainText('shared 12');
  await expect(row(LIVE)).toContainText('1 unproven');
  await expect(row(LIVE)).toContainText('gate PASS');
  await expect(row(LIVE)).toContainText('$0.42');
  await expect(row(LIVE)).toContainText('waiting for you: 1 gate ask');
  await expect(row(SQLGLOT)).toContainText('live in a terminal');
  await expect(row(SQLGLOT)).toContainText('2 challenges open');
  await expect(row(SQLGLOT)).toContainText('$0.03 + 2 unpriced');
  await expect(row(NOTES)).toContainText('idle');
  await expect(row(NOTES)).toContainText('no gate verdict yet');
  await expect(row(NOTES)).not.toContainText('$');
  await expect(row(GONE)).toContainText('missing');
  await expect(row(GONE)).toContainText('memory unknown');
});

test('a world overview says each source, and unknown where it is', async ({ page }) => {
  await open(page, `#/w/${NOTES.id}`);
  const panel = (t: string) => page.getByRole('region', { name: t });
  await expect(panel('Status')).toContainText('idle');
  await expect(panel('Last activity')).toContainText(
    'unknown — nothing has happened here that the board can read',
  );
  await expect(panel('Latest spend')).toContainText('unknown — no call is booked yet');
  await expect(panel('Challenges')).toContainText('no state directory');
  await expect(panel('Challenges')).toContainText('harness');
  await expect(panel('Memory')).toContainText('in-world, shown only');
  await expect(panel('End gate')).toContainText('No verdict yet.');

  await page.goto(`/#/w/${GONE.id}`);
  await expect(page.locator('.page-head')).toContainText('missing');
  await expect(page.getByText("This world's directory is gone.")).toBeVisible();
  await expect(panel('Memory')).toContainText('unknown — the directory is gone');
  await expect(panel('Challenges')).toContainText('pending');

  await page.goto(`/#/w/${SQLGLOT.id}`);
  await expect(page.getByText('too many entries: updates slowed')).toBeVisible();

  await page.goto(`/#/w/${LIVE.id}`);
  await expect(panel('Meters')).toContainText('precision 0.90 (18/20, 95% 0.70–0.97)');
  await expect(panel('Meters')).toContainText('gateRight no reports');
  await expect(page.getByText('Waiting for you: 1 gate ask')).toBeVisible();
});

test('a pushed world/changed is shown within a second', async ({ page }) => {
  const sockets: WebSocketRoute[] = [];
  const overviews: Record<string, BoardOverview> = structuredClone(fixtureOverviews);
  await open(page, `#/w/${LIVE.id}`, fixtureWorlds, overviews, sockets);
  await expect(page.locator('.page-live')).toHaveText('Updates as the world changes.');
  await expect(page.getByRole('region', { name: 'Last activity' })).toContainText('log.md written');
  const live = overviews[LIVE.id];
  if (!live) throw new Error('fixtures');
  live.activity = {
    value: { at: '2026-10-30T11:59:59Z', what: 'a run journal written' },
    source: '.mnemorph/instruments/loop/',
    trusted: false,
    why: null,
  };
  const [ws] = sockets;
  if (!ws) throw new Error('the page opened no push socket');
  const sent = Date.now();
  ws.send(JSON.stringify({ jsonrpc: '2.0', method: 'world/changed', params: { id: LIVE.id } }));
  await expect(page.getByRole('region', { name: 'Last activity' })).toContainText(
    'a run journal written',
    { timeout: 1000 },
  );
  expect(Date.now() - sent).toBeLessThan(1000);
  // Another world's change rereads nothing here.
  ws.send(JSON.stringify({ jsonrpc: '2.0', method: 'world/changed', params: { id: 'other' } }));
});

test('hostile names, paths and notes in an overview stay text', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const base = fixtureOverviews[LIVE.id];
  if (!base) throw new Error('fixtures');
  const world = {
    ...LIVE,
    id: 'evil-2',
    name: '<img src=x onerror="window.__pwned=1">\u202egnp',
    path: '/tmp/<script>window.__pwned=2</script>',
  };
  const evil: BoardOverview = structuredClone(base);
  evil.world = world;
  evil.status.why = '<b onmouseover="window.__pwned=3">run</b> \u001b[31m held';
  if (evil.gate.value?.last)
    evil.gate.value.last.verdict = '<a href="javascript:alert(1)">PASS</a>';
  if (evil.gate.value?.last) evil.gate.value.last.body = 'orc\u200bwire';
  if (evil.spend.value) evil.spend.value.session = '<svg onload=alert(1)>';
  evil.memory.source = 'javascript:window.__pwned=4';
  await open(page, '#/', { worlds: [world] }, { 'evil-2': evil });
  await expect(page.locator('.world-card')).toContainText('<img src=x');
  await expect(page.locator('.world-card .safe-esc').first()).toHaveText('\\u{202e}');
  await page.goto('/#/w/evil-2');
  await expect(page.getByRole('region', { name: 'Status' })).toContainText('<b onmouseover');
  await expect(page.getByRole('region', { name: 'Status' }).locator('.safe-esc')).toHaveText(
    '\\u{1b}',
  );
  await expect(page.getByRole('region', { name: 'End gate' })).toContainText('<a href=');
  await expect(page.getByRole('region', { name: 'End gate' }).locator('.safe-esc')).toHaveText(
    '\\u{200b}',
  );
  expect(
    await page
      .locator('main img, main script, main svg, main b, main a[href^="javascript"]')
      .count(),
  ).toBe(0);
  expect(await page.evaluate(() => (window as { __pwned?: number }).__pwned)).toBeUndefined();
  expect(errors).toEqual([]);
});
