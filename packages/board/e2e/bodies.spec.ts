/**
 * The bodies, queue, timeline and journal pages end to end, at phone and desktop in both themes:
 * screenshots, contrast and labels; every colony node labelled; a body card opened with its race
 * art, which loads by its exact name under the CSP; hostile text staying text.
 */
import { fileURLToPath } from 'node:url';
import { expect, type Page, test } from '@playwright/test';
import { fixtureBodies, fixtureQueue, journalOf, timelineOf } from '../src/data/bodiesFixtures.ts';
import { FIXTURE_NOW, fixtureOverviews, fixtureWorlds } from '../src/data/fixtures.ts';
import { lowContrast, unlabelled, violations } from './checks.ts';

const screen = (file: string) =>
  fileURLToPath(new URL(`../test-results/screens/${file}.png`, import.meta.url));

const W = fixtureWorlds.worlds.find((w) => w.name === 'sqlglot');
if (!W) throw new Error('the fixtures hold sqlglot');

async function open(page: Page, hash: string) {
  await page.addInitScript(() => {
    const seen: string[] = [];
    (window as { __csp?: string[] }).__csp = seen;
    document.addEventListener('securitypolicyviolation', (e) =>
      seen.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  await page.clock.setFixedTime(FIXTURE_NOW);
  const json = (body: unknown) => ({ contentType: 'application/json', body: JSON.stringify(body) });
  await page.route('**/api/worlds', (r) => r.fulfill(json(fixtureWorlds)));
  await page.route('**/api/session', (r) => r.fulfill(json({ csrf: 'ab'.repeat(32) })));
  await page.route('**/api/w/**', (r) => {
    const url = new URL(r.request().url());
    const [, , , id, part] = url.pathname.split('/');
    const q = (k: string) => url.searchParams.get(k);
    if (part === 'bodies') return r.fulfill(json(fixtureBodies));
    if (part === 'queue') return r.fulfill(json(fixtureQueue));
    if (part === 'timeline') return r.fulfill(json(timelineOf(q('kind'), q('body'))));
    if (part === 'journal') return r.fulfill(json(journalOf(q('q'))));
    const o = fixtureOverviews[decodeURIComponent(id ?? '')];
    return o ? r.fulfill(json(o)) : r.fulfill({ status: 404, body: 'no listed world has this id' });
  });
  await page.routeWebSocket('**/ws/board', () => {});
  await page.goto(`/${hash}`);
  await expect(page.locator('.shell')).toBeVisible();
}

const base = `#/w/${W.id}`;
const PAGES: [string, string, string][] = [
  ['colony', `${base}/bodies`, 'Pick a body to open its card.'],
  ['races', `${base}/bodies/races`, 'Arch Dark Elf'],
  ['queue', `${base}/bodies/queue`, 'fix the collate parse'],
  ['timeline', `${base}/bodies/timeline`, 'judge the dialect landing'],
  ['journal', `${base}/journal`, 'the dialect landing'],
];

async function checks(page: Page) {
  expect(await lowContrast(page)).toEqual([]);
  expect(await unlabelled(page)).toEqual([]);
  expect(await violations(page)).toEqual([]);
}

for (const theme of ['dark', 'light'] as const) {
  test.describe(`${theme} theme`, () => {
    test.use({ colorScheme: theme });

    for (const [name, hash, shows] of PAGES) {
      test(`${name}: screenshot, contrast, labels`, async ({ page }, info) => {
        await open(page, hash);
        await expect(page.getByText(shows).first()).toBeVisible();
        await page.screenshot({
          path: screen(`${info.project.name}-${theme}-${name}`),
          fullPage: true,
        });
        await checks(page);
      });
    }

    test('a body card opens with its race art, loaded by its exact name', async ({
      page,
    }, info) => {
      const art: string[] = [];
      page.on('response', (r) => {
        if (r.url().includes('/art/')) art.push(`${r.status()} ${new URL(r.url()).pathname}`);
      });
      await open(page, `${base}/bodies`);
      const nodes = page.locator('.colony-node');
      await expect(nodes).toHaveCount(fixtureBodies.bodies.length);
      // Every node carries its words: a label, and its name drawn.
      for (const n of await nodes.all()) {
        expect((await n.getAttribute('aria-label'))?.length).toBeGreaterThan(0);
        expect((await n.locator('.mark-label').textContent())?.trim()).not.toBe('');
      }
      await page.locator('.colony-node[data-name="orc-sql"]').focus();
      await page.keyboard.press('Enter');
      const card = page.locator('.body-card');
      await expect(card).toBeVisible();
      const img = card.locator('img');
      await expect(img).toHaveAttribute('src', '/art/cards/high-orc.webp');
      await expect
        .poll(() => img.evaluate((i: HTMLImageElement) => i.complete && i.naturalWidth))
        .toBeGreaterThan(0);
      expect(art).toContain('200 /art/cards/high-orc.webp');
      await card.scrollIntoViewIfNeeded();
      await page.screenshot({
        path: screen(`${info.project.name}-${theme}-body-card`),
        fullPage: true,
      });
      await checks(page);
    });

    test('hostile text stays text', async ({ page }) => {
      await open(page, `${base}/bodies/queue`);
      await expect(page.getByText('judge <script>alert(1)</script> the landing')).toBeVisible();
      await expect(page.locator('a[href^="javascript"]')).toHaveCount(0);
      await open(page, `${base}/journal`);
      await expect(page.getByText('<script>alert(1)</script>', { exact: false })).toBeVisible();
      await expect(page.locator('.journal-entry img')).toHaveCount(0);
      expect(await violations(page)).toEqual([]);
    });
  });
}

test('the journal searches, and its contents jump to an entry', async ({ page }) => {
  await open(page, `${base}/journal`);
  await page.getByRole('searchbox').fill('quoting');
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(page.locator('.journal-entry')).toHaveCount(1);
  await page.getByRole('button', { name: 'Clear' }).click();
  await expect(page.locator('.journal-entry')).toHaveCount(3);
  await page.locator('.journal-toc button').last().click();
  await expect(page.locator('.journal-entry').last()).toBeFocused();
});

test('the timeline filters by kind', async ({ page }) => {
  await open(page, `${base}/bodies/timeline`);
  await page.getByLabel('kind').selectOption('verdict');
  await expect(page.locator('.act[data-kind="commission"]')).toHaveCount(0);
  await expect(page.locator('.act[data-kind="verdict"]')).toHaveCount(3);
});
