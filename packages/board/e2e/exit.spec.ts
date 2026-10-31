/**
 * The M12 exit, on the board's real server, bridge and core, at phone and desktop in both themes:
 * the board serves every known world on loopback; the human chats; an act waits at the human gate
 * and is approved on the card under its protections — nothing answers it before it is armed —
 * and runs; memory, ontology, bodies and metrics are read, and move live as a body writes.
 *
 * The `board_e2e` example of mnemorph-server serves the built page over two fresh worlds (see its
 * docs); `board_e2e note|fact`, and a creature doc written into the world, land what a body would
 * while the page watches. CI builds the board,
 * then the example; locally the suite is skipped without it.
 */
import { type ChildProcess, execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, type Page, test } from '@playwright/test';
import { lowContrast } from './checks.ts';

const repo = fileURLToPath(new URL('../../../', import.meta.url));
const BIN =
  process.env.MNEMORPH_BOARD_E2E ??
  join(process.env.CARGO_TARGET_DIR ?? join(repo, 'target'), 'debug/examples/board_e2e');

const screen = (file: string) =>
  fileURLToPath(new URL(`../test-results/screens/${file}.png`, import.meta.url));

/** The rail, opened first at phone width, where it is a drawer. */
async function rail(page: Page) {
  const menu = page.getByRole('button', { name: 'Menu' });
  if (await menu.isVisible()) {
    if ((await menu.getAttribute('aria-expanded')) !== 'true') await menu.click();
    await expect(page.locator('#rail')).toBeVisible();
  }
  return page.locator('#rail');
}

for (const theme of ['dark', 'light'] as const) {
  test.describe(`the M12 exit, ${theme}`, () => {
    test.skip(!existsSync(BIN) && !process.env.CI, `no ${BIN}: build the board, then the example`);

    let root: string;
    let server: ChildProcess;
    let url: string;

    test.beforeEach(async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme });
      root = mkdtempSync(join(tmpdir(), 'mnemorph-board-exit-'));
      server = spawn(BIN, [root], { stdio: ['ignore', 'pipe', 'inherit'] });
      url = await new Promise<string>((resolve, reject) => {
        let out = '';
        server.stdout?.on('data', (d: Buffer) => {
          out += d.toString();
          const line = out.split('\n')[0];
          if (out.includes('\n') && line) resolve(line.trim());
        });
        server.on('exit', (code) => reject(new Error(`board_e2e exited with ${code}`)));
        setTimeout(() => reject(new Error('board_e2e printed no URL')), 30_000);
      });
    });

    test.afterEach(() => {
      server?.kill('SIGTERM');
      if (root) rmSync(root, { recursive: true, force: true });
    });

    test('every world on loopback; chat; approve on the card; memory, ontology, bodies and metrics live', async ({
      page,
      context,
    }, info) => {
      test.setTimeout(150_000);
      const shot = (name: string) =>
        page.screenshot({ path: screen(`exit-${info.project.name}-${theme}-${name}`) });
      const errors: string[] = [];
      page.on('pageerror', (e) => errors.push(e.message));
      await page.addInitScript(() => {
        const seen: string[] = [];
        (window as { __csp?: string[] }).__csp = seen;
        document.addEventListener('securitypolicyviolation', (e) =>
          seen.push(`${e.violatedDirective} ${e.blockedURI}`),
        );
      });

      // Every known world, served on loopback only.
      expect(new URL(url).hostname).toBe('127.0.0.1');
      await page.goto(url);
      const cards = page.locator('.world-card');
      await expect(cards).toHaveCount(2);
      await expect(cards.filter({ hasText: 'atelier' })).toBeVisible();
      await expect(cards.filter({ hasText: /^\W*world\b/ })).toBeVisible();
      expect(page.url()).not.toContain('#k='); // the key left the address bar
      await shot('worlds');
      await cards.filter({ hasText: 'atelier' }).click();
      await expect(page).toHaveURL(/#\/w\/[^/]+$/);
      const id = decodeURIComponent(new URL(page.url()).hash.split('/')[2] ?? '');
      const at = (path: string) => `#/w/${encodeURIComponent(id)}${path}`;
      const worldDir = join(root, 'atelier');

      // Memory: what is there, then a note a body lands, shown live (the board's watcher, warm).
      await page.goto(`${new URL(url).origin}/${at('/memory')}`);
      await expect(page.locator('main')).toContainText('the notes file holds one line');
      execFileSync(BIN, ['note', worldDir, 'orc-notes', 'a second note, landed live']);
      await expect(page.locator('main')).toContainText('a second note, landed live', {
        timeout: 15_000,
      });

      // Ontology: the fact, then one more, live.
      await page.goto(`${new URL(url).origin}/${at('/ontology')}`);
      await page.getByRole('tab', { name: /single/ }).click();
      await expect(page.locator('main')).toContainText('the notes are plain text');
      execFileSync(BIN, ['fact', worldDir, 'notes.txt', 'the board reads it live']);
      await expect(page.locator('main')).toContainText('the board reads it live', {
        timeout: 15_000,
      });

      // Metrics, open in a second tab of this browser before the chat: it moves when the chat spends.
      const metrics = await context.newPage();
      await metrics.emulateMedia({ colorScheme: theme });
      await metrics.goto(`${new URL(url).origin}/#/metrics`);
      await expect(metrics.locator('main')).toContainText('Metrics');
      await expect(metrics.locator('main')).toContainText('no calls');

      // Chat, and approve under the card's protections.
      await (await rail(page)).getByRole('button', { name: '+ New chat' }).click();
      await expect(page).toHaveURL(/#\/w\/[^/]+\/s\/[^/]+$/);
      const box = page.getByRole('textbox');
      await expect(box).toBeEnabled({ timeout: 30_000 });
      await box.fill('change a to b in the notes');
      await box.press('Enter');
      const card = page.getByRole('alertdialog');
      await expect(card).toBeVisible({ timeout: 30_000 });
      await expect(card).toContainText('sed -i s/a/b/ notes.txt');
      // A key before the card is armed answers nothing; Yes is never where the focus starts.
      await page.keyboard.press('Enter');
      await expect(card).toBeVisible();
      const yes = card.getByRole('button', { name: /Yes, run it once/ });
      await expect(yes).not.toBeFocused();
      // Neither the palette nor the map opens over it.
      await page.keyboard.press('Control+k');
      await page.keyboard.press('?');
      await expect(page.getByRole('dialog', { name: 'Command palette' })).toHaveCount(0);
      await expect(page.getByRole('dialog', { name: 'Keyboard shortcuts' })).toHaveCount(0);
      expect(readFileSync(join(worldDir, 'notes.txt'), 'utf8')).toBe('a\n');
      await expect(card).toHaveAttribute('data-armed', 'true');
      await expect(yes).toBeEnabled();
      await shot('approval');
      await yes.click();
      await expect(card).toHaveCount(0);
      await expect(page.locator('main')).toContainText('the human approved the edit', {
        timeout: 30_000,
      });
      expect(readFileSync(join(worldDir, 'notes.txt'), 'utf8')).toBe('b\n');
      await shot('chat');

      // The metrics tab moved without a reload: the chat's calls, unpriced, never $0.
      await expect(metrics.locator('main')).toContainText('2 calls', { timeout: 20_000 });
      await expect(metrics.locator('main')).toContainText('unpriced');
      await expect(metrics.locator('main')).not.toContainText('$0.00');
      await metrics.screenshot({ path: screen(`exit-${info.project.name}-${theme}-metrics`) });
      await metrics.close();

      // Bodies: the colony from the world's docs, and the race law compiled in.
      await page.goto(`${new URL(url).origin}/${at('/bodies')}`);
      await expect(page.locator('.colony-node[data-name="orc-notes"]')).toBeVisible();
      // A body writes a new creature's doc: the colony grows, live.
      mkdirSync(join(worldDir, '.mnemorph/slime/slime-notes'), { recursive: true });
      writeFileSync(
        join(worldDir, '.mnemorph/slime/slime-notes/README.md'),
        '# slime-notes\n\n- **Race:** slime\n- **Reports to:** orc-notes\n- **Territory:** docs/\n',
      );
      await expect(page.locator('.colony-node[data-name="slime-notes"]')).toBeVisible({
        timeout: 15_000,
      });
      await shot('bodies');
      await page.goto(`${new URL(url).origin}/${at('/bodies/races')}`);
      await expect(page.locator('main')).toContainText('Slime');

      // The journal, rendered.
      await page.goto(`${new URL(url).origin}/${at('/journal')}`);
      await expect(page.locator('main')).toContainText('the atelier opens');

      expect(await lowContrast(page)).toEqual([]);
      expect(errors).toEqual([]);
      expect(await page.evaluate(() => (window as { __csp?: string[] }).__csp)).toEqual([]);
    });
  });
}
