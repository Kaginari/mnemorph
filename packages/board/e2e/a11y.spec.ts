/**
 * Accessibility end to end, at phone and desktop in both themes: axe (WCAG 2.2 A and AA rules) on
 * every page of the board — the worlds, a world, its chats, the approval card, memory, ontology,
 * bodies and every tab of theirs, the journal, metrics, integrations, settings, an unknown
 * address, the command palette and the shortcut map, and the empty and loading states — with no
 * serious or critical violation. And the keyboard: the palette is focus-trapped and closes on Esc,
 * jumps by its keys, never opens over an approval card; the rail walks by the arrows; the live
 * regions say an answer and an ask once.
 */
import AxeBuilder from '@axe-core/playwright';
import { expect, type Page, test } from '@playwright/test';
import { fixtureBodies, fixtureQueue, journalOf, timelineOf } from '../src/data/bodiesFixtures.ts';
import { FIXTURE_NOW, fixtureOverviews, fixtureWorlds } from '../src/data/fixtures.ts';
import { factsOf, fixtureMemory } from '../src/data/knowledgeFixtures.ts';
import { fixtureIntegrations, fixtureMetrics } from '../src/data/metricsFixtures.ts';
import { askLine, Bridge, CHAT, LIVE, openChat, richTurn, runs } from './fake.ts';

const W = fixtureWorlds.worlds.find((w) => w.name === 'sqlglot');
if (!W) throw new Error('the fixtures hold sqlglot');

const json = (body: unknown) => ({ contentType: 'application/json', body: JSON.stringify(body) });

/** Every read the board makes, answered from the fixtures; `worlds` stands in for the list. */
async function routeAll(page: Page, worlds: unknown = fixtureWorlds) {
  await page.clock.setFixedTime(FIXTURE_NOW);
  await page.route('**/api/worlds', (r) => r.fulfill(json(worlds)));
  await page.route('**/api/session', (r) => r.fulfill(json({ csrf: 'ab'.repeat(32) })));
  await page.route('**/api/sessions', (r) => r.fulfill(json({ sessions: [] })));
  await page.route('**/api/metrics?*', (r) => r.fulfill(json(fixtureMetrics)));
  await page.route('**/api/integrations*', (r) => r.fulfill(json(fixtureIntegrations)));
  await page.route('**/api/w/**', (r) => {
    const url = new URL(r.request().url());
    const [, , , id, part] = url.pathname.split('/');
    const q = (k: string) => url.searchParams.get(k);
    const answers: Record<string, () => unknown> = {
      memory: () => fixtureMemory,
      facts: () => factsOf(q('status')),
      bodies: () => fixtureBodies,
      queue: () => fixtureQueue,
      timeline: () => timelineOf(q('kind'), q('body')),
      journal: () => journalOf(q('q')),
      runs: () => runs,
    };
    const answer = part === undefined ? undefined : answers[part];
    if (answer) return r.fulfill(json(answer()));
    const o = fixtureOverviews[decodeURIComponent(id ?? '')];
    return o ? r.fulfill(json(o)) : r.fulfill({ status: 404, body: 'no listed world has this id' });
  });
  await page.routeWebSocket('**/ws/board', () => {});
}

/** Axe on the page as it stands: the serious and critical violations, by rule and node. */
async function axe(page: Page): Promise<string[]> {
  const r = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa', 'best-practice'])
    .analyze();
  return r.violations
    .filter((v) => v.impact === 'serious' || v.impact === 'critical')
    .flatMap((v) =>
      v.nodes.map(
        (n) => `${v.id} (${v.impact}): ${n.target.join(' ')} — ${n.failureSummary ?? ''}`,
      ),
    );
}

const PAGES: [string, string, string][] = [
  ['worlds', '#/', 'tempest-try'],
  ['world', `#/w/${W.id}`, 'sqlglot'],
  ['memory', `#/w/${W.id}/memory`, 'collations drop in _parse_types'],
  ['ontology', `#/w/${W.id}/ontology`, 'quoting is the tokenizer’s'],
  ['colony', `#/w/${W.id}/bodies`, 'orc-sql'],
  ['races', `#/w/${W.id}/bodies/races`, 'Slime'],
  ['queue', `#/w/${W.id}/bodies/queue`, 'Queue'],
  ['timeline', `#/w/${W.id}/bodies/timeline`, 'Timeline'],
  ['journal', `#/w/${W.id}/journal`, 'Journal'],
  ['metrics', '#/metrics', 'Metrics'],
  ['integrations', '#/integrations', 'Integrations'],
  ['settings', '#/settings', 'Settings'],
  ['unknown', '#/nowhere', 'Nothing at this address'],
];

for (const theme of ['dark', 'light'] as const) {
  test.describe(`axe, ${theme}`, () => {
    test.beforeEach(async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme });
    });

    for (const [name, hash, text] of PAGES) {
      test(`${name}: no serious or critical violation`, async ({ page }) => {
        await routeAll(page);
        await page.goto(`/${hash}`);
        await expect(page.locator('main')).toContainText(text);
        expect(await axe(page)).toEqual([]);
      });
    }

    test('the empty board, the loading state and the open menu', async ({ page }, info) => {
      await routeAll(page, { worlds: [], unreadable: 0 });
      await page.goto('/#/');
      await expect(page.locator('main')).toContainText('No worlds yet');
      await expect(page.locator('main')).toContainText('mnemorphd board add <dir>');
      expect(await axe(page)).toEqual([]);
      if (info.project.name === 'phone') {
        await page.getByRole('button', { name: 'Menu' }).click();
        await expect(page.locator('#rail')).toBeVisible();
        expect(await axe(page)).toEqual([]);
      }
      // Loading: the list never comes.
      await page.unroute('**/api/worlds');
      await page.route('**/api/worlds', () => {});
      await page.reload();
      await expect(page.locator('main')).toContainText('Loading worlds…');
      expect(await axe(page)).toEqual([]);
    });

    test('a chat, its live regions and an approval card', async ({ page }) => {
      const b = new Bridge();
      richTurn(b);
      await openChat(page, b);
      await expect(page.locator('.tool-name').first()).toBeVisible();
      expect(await axe(page)).toEqual([]);
      // A turn ends live: its answer is said once, politely.
      b.emit('turnStarted', { text: 'again' });
      b.emit('assistant', { text: 'The second answer.', stop: 'end_turn' });
      b.emit('turnEnded', { ending: 'answered' });
      await expect(page.getByTestId('announce-polite')).toContainText('The second answer.');
      // An ask: said at once, and the card holds the page.
      b.say(
        askLine('gate-1', {
          body: 'orc-wire',
          race: 'orc',
          class: 'write',
          act: "sed -i 's/a/b/' crates/wire/src/lib.rs",
          why: ['writes a file in its zone'],
          outside: [],
        }),
      );
      await expect(page.getByRole('alertdialog')).toBeVisible();
      await expect(page.getByTestId('announce-assertive')).toContainText(
        'An ask waits at the gate',
      );
      expect(await axe(page)).toEqual([]);
    });

    test('the palette and the shortcut map', async ({ page }) => {
      await routeAll(page);
      await page.goto('/#/');
      await expect(page.locator('main')).toContainText('tempest-try');
      await page.keyboard.press('Control+k');
      await expect(page.getByRole('dialog', { name: 'Command palette' })).toBeVisible();
      expect(await axe(page)).toEqual([]);
      await page.keyboard.press('Escape');
      await page.keyboard.press('?');
      await expect(page.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeVisible();
      expect(await axe(page)).toEqual([]);
    });
  });
}

test('the palette: jumps by its keys, holds the focus, closes on Esc and gives it back', async ({
  page,
}) => {
  await routeAll(page);
  await page.goto('/#/');
  await expect(page.locator('main')).toContainText('tempest-try');
  await page.keyboard.press('Control+k');
  const dialog = page.getByRole('dialog', { name: 'Command palette' });
  const box = page.getByRole('combobox');
  await expect(box).toBeFocused();
  await box.fill('memory sqlglot');
  await expect(dialog.getByRole('option').first()).toContainText('Memory');
  await expect(dialog.getByRole('option').first()).toHaveAttribute('aria-selected', 'true');
  // Tab never leaves it.
  for (let i = 0; i < 5; i++) {
    await page.keyboard.press('Tab');
    expect(await dialog.evaluate((d) => d.contains(document.activeElement))).toBe(true);
  }
  await box.focus();
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL((u) => u.hash === `#/w/${W.id}/memory`);
  await expect(dialog).toHaveCount(0);

  // Meta+K too; ↓ moves; Esc closes and the focus is back on what had it.
  await page.getByRole('button', { name: /^Search/ }).focus();
  await page.keyboard.press('Meta+k');
  await expect(dialog).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await expect(dialog.getByRole('option').nth(1)).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole('button', { name: /^Search/ })).toBeFocused();
});

test('the palette never opens over an approval card, nor answers one', async ({ page }) => {
  const b = new Bridge();
  b.emit('turnStarted', { text: 'tidy the wire' });
  await openChat(page, b);
  await expect(page.locator('.msg-user')).toHaveCount(1);
  // Open first, then an ask arrives: the palette goes, the card stays.
  await page.keyboard.press('Control+k');
  await expect(page.getByRole('dialog', { name: 'Command palette' })).toBeVisible();
  b.say(
    askLine('gate-1', {
      body: 'orc-wire',
      race: 'orc',
      class: 'write',
      act: "sed -i 's/a/b/' crates/wire/src/lib.rs",
      why: ['writes a file in its zone'],
      outside: [],
    }),
  );
  await expect(page.getByRole('alertdialog')).toBeVisible();
  await expect(page.getByRole('dialog', { name: 'Command palette' })).toHaveCount(0);
  // With the card up, neither shortcut opens anything, and Enter after them answers nothing.
  await page.keyboard.press('Control+k');
  await page.keyboard.press('?');
  await expect(page.getByRole('dialog', { name: 'Command palette' })).toHaveCount(0);
  await expect(page.getByRole('dialog', { name: 'Keyboard shortcuts' })).toHaveCount(0);
  await page.waitForTimeout(400);
  const allows = b.sent.filter(
    (m) => m.method === undefined && (m.result as { allow?: boolean } | undefined)?.allow === true,
  );
  expect(allows).toEqual([]);
  expect(page.url()).toContain(`/s/${CHAT}`);
  expect(LIVE.id).toBeTruthy();
});

test('the rail walks by the arrows', async ({ page }, info) => {
  test.skip(info.project.name !== 'desktop', 'the rail is a drawer at phone width');
  await routeAll(page);
  await page.goto(`/#/w/${W.id}`);
  await expect(page.locator('main')).toContainText('sqlglot');
  const items = page.locator('#rail').locator('a[href], button:not([disabled])');
  await items.first().focus();
  await page.keyboard.press('ArrowDown');
  await expect(items.nth(1)).toBeFocused();
  await page.keyboard.press('End');
  await expect(items.last()).toBeFocused();
  await page.keyboard.press('Home');
  await expect(items.first()).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(items.last()).toBeFocused();
});

test('reduced motion: nothing on the board animates', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await routeAll(page);
  await page.goto('/#/');
  await expect(page.locator('main')).toContainText('tempest-try');
  const moving = await page.evaluate(
    () =>
      [...document.querySelectorAll('*')].filter((el) => {
        const cs = getComputedStyle(el);
        const secs = (s: string) => Math.max(...s.split(',').map((x) => Number.parseFloat(x) || 0));
        return secs(cs.animationDuration) > 0.001 || secs(cs.transitionDuration) > 0.001;
      }).length,
  );
  expect(moving).toBe(0);
});
