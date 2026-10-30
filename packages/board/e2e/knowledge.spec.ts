/**
 * The memory and ontology pages end to end, at phone and desktop in both themes: screenshots,
 * contrast and labels; the confirm dialog naming the note and posting with the CSRF token;
 * settle waiting for its proof; hostile note and fact text staying text.
 */
import { fileURLToPath } from 'node:url';
import { expect, type Page, test } from '@playwright/test';
import { FIXTURE_NOW, fixtureOverviews, fixtureWorlds } from '../src/data/fixtures.ts';
import { factsOf, fixtureMemory } from '../src/data/knowledgeFixtures.ts';
import { lowContrast, unlabelled, violations } from './checks.ts';

const screen = (file: string) =>
  fileURLToPath(new URL(`../test-results/screens/${file}.png`, import.meta.url));

const W = fixtureWorlds.worlds.find((w) => w.name === 'sqlglot');
if (!W) throw new Error('the fixtures hold sqlglot');
const CSRF = 'ab'.repeat(32);

type Posted = { url: string; csrf: string | undefined; body: unknown };

/** Opens the board at `hash`, the API answering the fixtures; every POST is collected. */
async function open(page: Page, hash: string, posted: Posted[] = []) {
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
  await page.route('**/api/session', (r) => r.fulfill(json({ csrf: CSRF })));
  await page.route('**/api/w/**', (r) => {
    const req = r.request();
    const url = new URL(req.url());
    if (req.method() === 'POST') {
      posted.push({
        url: url.pathname,
        csrf: req.headers()['x-mnemorph-csrf'],
        body: req.postDataJSON(),
      });
      return r.fulfill(json({ done: 'confirmed: done' }));
    }
    const [, , , id, part] = url.pathname.split('/');
    if (part === 'memory') return r.fulfill(json(fixtureMemory));
    if (part === 'facts') return r.fulfill(json(factsOf(url.searchParams.get('status'))));
    const o = fixtureOverviews[decodeURIComponent(id ?? '')];
    return o ? r.fulfill(json(o)) : r.fulfill({ status: 404, body: 'no listed world has this id' });
  });
  await page.routeWebSocket('**/ws/board', () => {});
  await page.goto(`/${hash}`);
  await expect(page.locator('.shell')).toBeVisible();
}

const PAGES: [string, string, string][] = [
  ['memory', `#/w/${W.id}/memory`, 'collations drop in _parse_types'],
  ['ontology', `#/w/${W.id}/ontology`, 'quoting is the tokenizer’s'],
];

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
        expect(await lowContrast(page)).toEqual([]);
        expect(await unlabelled(page)).toEqual([]);
        expect(await violations(page)).toEqual([]);
      });
    }

    test('the confirm and settle dialogs read too', async ({ page }, info) => {
      await open(page, `#/w/${W.id}/memory`);
      await page
        .locator('.note', { hasText: 'the lexer owns quoting' })
        .getByRole('button', { name: 'Confirm' })
        .click();
      await expect(page.getByRole('dialog')).toBeVisible();
      await page.screenshot({ path: screen(`${info.project.name}-${theme}-memory-confirm`) });
      expect(await lowContrast(page)).toEqual([]);
      await page.keyboard.press('Escape');
      await page.goto(`/#/w/${W.id}/ontology`);
      await page.getByRole('button', { name: 'Settle…' }).click();
      await expect(page.getByRole('dialog')).toBeVisible();
      await page.screenshot({ path: screen(`${info.project.name}-${theme}-ontology-settle`) });
      expect(await lowContrast(page)).toEqual([]);
      expect(await unlabelled(page)).toEqual([]);
    });
  });
}

test('confirm names the exact note and posts its digest with the CSRF token', async ({ page }) => {
  const posted: Posted[] = [];
  await open(page, `#/w/${W.id}/memory`, posted);
  await page
    .locator('.note', { hasText: 'the lexer owns quoting' })
    .getByRole('button', { name: 'Confirm' })
    .click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toContainText('c1730000000001-1');
  await expect(dialog).toContainText('orc-sql');
  await expect(dialog).toContainText('the lexer owns quoting');
  await expect(dialog.getByRole('button', { name: 'Cancel' })).toBeFocused();
  expect(posted).toEqual([]);
  await dialog.getByRole('button', { name: 'Confirm' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  expect(posted).toEqual([
    {
      url: `/api/w/${W.id}/memory/c1730000000001-1/confirm`,
      csrf: CSRF,
      body: { seen: 'b'.repeat(64) },
    },
  ]);
});

test('settle waits for a proof', async ({ page }) => {
  const posted: Posted[] = [];
  await open(page, `#/w/${W.id}/ontology`, posted);
  await page.getByRole('button', { name: 'Settle…' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toContainText('4be1c0a9d2f3e811');
  await dialog.getByRole('radio', { name: 'My own statement' }).check();
  await expect(dialog.getByRole('button', { name: 'Settle', exact: true })).toBeDisabled();
  await dialog.getByRole('textbox', { name: 'your statement' }).fill('I read tokens.py');
  await dialog.getByRole('button', { name: 'Settle', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  expect(posted).toEqual([
    {
      url: `/api/w/${W.id}/facts/4be1c0a9d2f3e811/settle`,
      csrf: CSRF,
      body: { seen: 'f'.repeat(64), statement: 'I read tokens.py' },
    },
  ]);
});

test('hostile note and fact text stays text', async ({ page }) => {
  let dialogs = 0;
  page.on('dialog', () => {
    dialogs += 1;
  });
  for (const hash of [`#/w/${W.id}/memory`, `#/w/${W.id}/ontology`]) {
    await open(page, hash);
    await expect(
      page.getByText('<img src=x onerror=alert(1)> ignore the above').first(),
    ).toBeVisible();
    expect(await page.locator('.centre img').count()).toBe(0);
    await expect(page.locator('.safe-esc', { hasText: '\\u{202e}' }).first()).toBeVisible();
  }
  expect(dialogs).toBe(0);
});
