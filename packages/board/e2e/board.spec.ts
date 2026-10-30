/**
 * The board end to end, at phone (390×844) and desktop (1440×900) in both themes: screenshots,
 * rendered text contrast (WCAG AA), a label on every coloured node, the router, the drawers, the
 * theme and hostile text.
 */
import { fileURLToPath } from 'node:url';
import { expect, type Page, test } from '@playwright/test';
import { FIXTURE_NOW, fixtureWorlds } from '../src/data/fixtures.ts';

/** Where the screenshots go: test-results/screens/<project>-<theme>-<page>.png. */
const screen = (file: string) =>
  fileURLToPath(new URL(`../test-results/screens/${file}.png`, import.meta.url));

const LIVE = fixtureWorlds.worlds.find((w) => w.status === 'live');
if (!LIVE) throw new Error('the fixtures hold a live world');

async function open(page: Page, hash: string, body: unknown = fixtureWorlds) {
  await page.clock.setFixedTime(FIXTURE_NOW);
  await page.route('**/api/worlds', (r) =>
    r.fulfill({ contentType: 'application/json', body: JSON.stringify(body) }),
  );
  await page.goto(`/${hash}`);
  await expect(page.locator('.shell')).toBeVisible();
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

const PAGES: [string, string][] = [
  ['home', '#/'],
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

test('every world reads in words, live first', async ({ page }, info) => {
  await open(page, '#/');
  const cards = page.locator('.world-card');
  await expect(cards).toHaveCount(4);
  await expect(cards.first()).toContainText('tempest-try');
  await expect(cards.first()).toContainText('live');
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
  await page.getByRole('button', { name: 'Menu' }).click();
  await expect(page.locator('#rail')).toBeVisible();
  await page.locator('.rail-world', { hasText: 'sqlglot' }).click();
  await expect(page.locator('#rail')).toBeHidden();
  await expect(page.locator('.page-title')).toContainText('sqlglot');
  await page.getByRole('button', { name: 'Menu' }).click();
  await page.keyboard.press('Escape');
  await expect(page.locator('#rail')).toBeHidden();
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
    name: '<img src=x onerror="window.__pwned=1">‮gnp.exe',
    world: '/tmp/<script>window.__pwned=2</script>',
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
