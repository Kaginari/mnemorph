/**
 * A chat end to end on the board's real server and bridge: the `board_e2e` example of
 * mnemorph-server serves this built page over one fresh world, and each chat's child is a real
 * core — the loop, the tools, the end gate — answering from a scripted model instead of a
 * provider. The page trades the printed key, opens a chat, sends two turns and sees them in the
 * TUI's grammar, under the server's own CSP; a reload replays without duplicating; the past run
 * reads from its journal once the chat ends.
 *
 * The binary is built by `cargo build -p mnemorph-server --example board_e2e` after the board
 * (`pnpm --filter @mnemorph/board build`): CI does both. Locally, the test is skipped without it.
 */
import { type ChildProcess, spawn } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

const repo = fileURLToPath(new URL('../../../', import.meta.url));
const BIN =
  process.env.MNEMORPH_BOARD_E2E ??
  join(process.env.CARGO_TARGET_DIR ?? join(repo, 'target'), 'debug/examples/board_e2e');

test.describe('the real bridge', () => {
  test.skip(!existsSync(BIN) && !process.env.CI, `no ${BIN}: build the board, then the example`);

  let root: string;
  let server: ChildProcess;
  let url: string;

  test.beforeEach(async () => {
    test.skip(test.info().project.name !== 'desktop', 'one browser is enough');
    root = mkdtempSync(join(tmpdir(), 'mnemorph-board-e2e-'));
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

  test('a scripted session, two turns, through the real bridge', async ({ page }) => {
    test.setTimeout(90_000);
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.addInitScript(() => {
      const seen: string[] = [];
      (window as { __csp?: string[] }).__csp = seen;
      document.addEventListener('securitypolicyviolation', (e) =>
        seen.push(`${e.violatedDirective} ${e.blockedURI}`),
      );
    });
    await page.goto(url);
    const world = page.locator('.world-card', { hasText: 'world' });
    await world.click();
    await page.getByRole('button', { name: '+ New chat' }).click();
    await expect(page).toHaveURL(/#\/w\/[^/]+\/s\/[^/]+$/);
    await expect(page.locator('.chat-title')).toContainText('scripted-model');
    const box = page.getByRole('textbox');
    await expect(box).toBeEnabled();
    await box.fill('what does notes.txt say?');
    await box.press('Enter');

    await expect(page.locator('.msg-user')).toContainText('what does notes.txt say?');
    await expect(page.locator('.tool-name', { hasText: 'edit' })).toBeVisible({ timeout: 30_000 });
    await expect(page.locator('.diff-add .diff-text')).toHaveText('b');
    await expect(page.locator('.diff-del .diff-text')).toHaveText('a');
    await expect(page.locator('main')).toContainText('they say b now');
    await expect(page.locator('.verdict')).toBeVisible();
    // The model's hostile tail stays text.
    await expect(page.locator('main')).toContainText('(link not followed)');
    expect(await page.locator('main img, main a[href^="javascript"]').count()).toBe(0);
    await expect(page.locator('.activity')).toHaveCount(0);

    await box.fill('and again');
    await box.press('Enter');
    await expect(
      page.locator('.msg-body', { hasText: 'The second turn, answered.' }),
    ).toBeVisible();
    await expect(page.locator('.msg-user')).toHaveCount(2);
    await expect(page.locator('#pane .file')).toContainText('notes.txt');

    // A reload replays the chat from the bridge: each message once.
    await page.reload();
    await expect(
      page.locator('.msg-body', { hasText: 'The second turn, answered.' }),
    ).toBeVisible();
    await expect(page.locator('.msg-user')).toHaveCount(2);
    await expect(page.locator('.tool-name', { hasText: 'edit' })).toHaveCount(1);

    // Ended, the chat is a past run, read from its journal.
    await page.getByRole('button', { name: 'End chat' }).click();
    await expect(page.locator('.composer-closed')).toBeVisible();
    const past = page.locator('a.rail-chat[href*="/r/"]', { hasText: 'and again' });
    await expect(past).toBeVisible({ timeout: 20_000 });
    await past.click();
    await expect(page).toHaveURL(/#\/w\/[^/]+\/r\/r\d+-\d+$/);
    await expect(page.locator('.msg-user')).toHaveCount(2);
    await expect(
      page.locator('.msg-body', { hasText: 'The second turn, answered.' }),
    ).toBeVisible();
    await expect(page.locator('.composer-closed')).toContainText('read-only');

    expect(errors).toEqual([]);
    expect(await page.evaluate(() => (window as { __csp?: string[] }).__csp)).toEqual([]);
  });
});
