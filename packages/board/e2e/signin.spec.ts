/**
 * A tab without a session is told so, never shown a blank page: the page's own signed-out and
 * spent-link screens (under the board server's CSP, the API routed), and the server's refusal
 * pages for a foreign host name and a guarded path loaded without a session (on the real server,
 * the `board_e2e` example). Screenshots land in test-results/screens/signin-*.png.
 */
import { type ChildProcess, spawn } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { type Browser, chromium, expect, type Page, test } from '@playwright/test';
import { fixtureWorlds } from '../src/data/fixtures.ts';

const screen = (file: string) =>
  fileURLToPath(new URL(`../test-results/screens/${file}.png`, import.meta.url));

/** Collect every CSP violation the page reports; `seen` reads them. */
async function watchCsp(page: Page) {
  await page.addInitScript(() => {
    const seen: string[] = [];
    (window as { __csp?: string[] }).__csp = seen;
    document.addEventListener('securitypolicyviolation', (e) =>
      seen.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
}
const seen = (page: Page) => page.evaluate(() => (window as { __csp?: string[] }).__csp ?? []);

const KEY = 'ab'.repeat(32);

test.describe('the page, signed out', () => {
  test('a 401 from the API shows the signed-out page, not a blank root', async ({ page }, info) => {
    await watchCsp(page);
    await page.route('**/api/**', (r) =>
      r.fulfill({ status: 401, contentType: 'text/plain', body: 'no session' }),
    );
    await page.routeWebSocket('**/ws/board', (ws) => ws.close({ code: 4401 }));
    await page.goto('/#/');
    await expect(page.getByRole('heading', { name: 'This tab isn’t signed in' })).toBeVisible();
    await expect(page.getByText('Each link works once.')).toBeVisible();
    await expect(page.locator('.shell')).toHaveCount(0);
    await page.screenshot({ path: screen(`signin-${info.project.name}-signed-out`) });
    expect(await seen(page)).toEqual([]);
  });

  test('a spent link says so', async ({ page }, info) => {
    await watchCsp(page);
    await page.route('**/session', (r) =>
      r.fulfill({ status: 401, contentType: 'text/plain', body: 'the key is not valid' }),
    );
    await page.route('**/api/**', (r) =>
      r.fulfill({ status: 401, contentType: 'text/plain', body: 'no session' }),
    );
    await page.routeWebSocket('**/ws/board', (ws) => ws.close({ code: 4401 }));
    await page.goto(`/#k=${KEY}`);
    await expect(page.getByRole('heading', { name: 'This link was already used' })).toBeVisible();
    await expect(page.getByText('such as VS Code’s')).toBeVisible();
    expect(page.url()).not.toContain(KEY);
    await page.screenshot({ path: screen(`signin-${info.project.name}-link-used`) });
    expect(await seen(page)).toEqual([]);
  });

  test('a spent link in a tab that still holds a session goes on to the board', async ({
    page,
  }) => {
    await page.route('**/session', (r) =>
      r.fulfill({ status: 401, contentType: 'text/plain', body: 'the key is not valid' }),
    );
    await page.route('**/api/session', (r) =>
      r.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({ csrf: 'cd'.repeat(32) }),
      }),
    );
    await page.route('**/api/worlds', (r) =>
      r.fulfill({ contentType: 'application/json', body: JSON.stringify(fixtureWorlds) }),
    );
    await page.route('**/api/w/*', (r) => r.fulfill({ status: 404, body: 'no' }));
    await page.routeWebSocket('**/ws/board', () => {});
    await page.goto(`/#k=${KEY}`);
    await expect(page.locator('.shell')).toBeAttached();
    await expect(page.locator('.world-card').first()).toBeVisible();
  });
});

const repo = fileURLToPath(new URL('../../../', import.meta.url));
const BIN =
  process.env.MNEMORPH_BOARD_E2E ??
  join(process.env.CARGO_TARGET_DIR ?? join(repo, 'target'), 'debug/examples/board_e2e');

test.describe('the server, refusing a page load', () => {
  test.skip(!existsSync(BIN) && !process.env.CI, `no ${BIN}: build the board, then the example`);

  let root: string;
  let server: ChildProcess;
  let port: string;
  let browser: Browser;

  test.beforeEach(async () => {
    test.skip(test.info().project.name !== 'desktop', 'one browser is enough');
    root = mkdtempSync(join(tmpdir(), 'mnemorph-board-signin-'));
    server = spawn(BIN, [root], { stdio: ['ignore', 'pipe', 'inherit'] });
    const url = await new Promise<string>((resolve, reject) => {
      let out = '';
      server.stdout?.on('data', (d: Buffer) => {
        out += d.toString();
        const line = out.split('\n')[0];
        if (out.includes('\n') && line) resolve(line.trim());
      });
      server.on('exit', (code) => reject(new Error(`board_e2e exited with ${code}`)));
      setTimeout(() => reject(new Error('board_e2e printed no URL')), 30_000);
    });
    port = new URL(url).port;
    // A name of the test's own pointed at loopback, as a rebinding page's would be.
    browser = await chromium.launch({
      args: ['--host-resolver-rules=MAP mnemorph.test 127.0.0.1'],
    });
  });

  test.afterEach(async () => {
    await browser?.close();
    server?.kill('SIGTERM');
    if (root) rmSync(root, { recursive: true, force: true });
  });

  test('a foreign host name gets a page naming the board’s addresses', async () => {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    await watchCsp(page);
    const res = await page.goto(`http://mnemorph.test:${port}/`);
    expect(res?.status()).toBe(403);
    expect(res?.headers()['content-security-policy']).toContain("script-src 'self'");
    await expect(page.getByRole('heading', { name: 'This address is refused' })).toBeVisible();
    await expect(page.getByText('to block DNS rebinding')).toBeVisible();
    // Each bound address is listed (`[::1]` too where the machine has it), and `localhost`.
    for (const at of [`127.0.0.1:${port}`, `localhost:${port}`]) {
      await expect(
        page.getByRole('listitem').getByRole('link', { name: `http://${at}`, exact: true }),
      ).toBeVisible();
    }
    // Styled: the stylesheet, served to every host, loaded under the CSP.
    const top = await page.locator('.refused').evaluate((e) => getComputedStyle(e).borderTopWidth);
    expect(top).toBe('3px');
    await page.screenshot({ path: screen('signin-desktop-host') });
    expect(await seen(page)).toEqual([]);
  });

  test('a guarded path loaded without a session gets the signed-out page', async () => {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    await watchCsp(page);
    const res = await page.goto(`http://127.0.0.1:${port}/api/worlds`);
    expect(res?.status()).toBe(401);
    await expect(page.getByRole('heading', { name: 'This tab isn’t signed in' })).toBeVisible();
    await page.screenshot({ path: screen('signin-desktop-server-signed-out') });
    expect(await seen(page)).toEqual([]);
  });

  test('localhost is answered, and signs in on its own', async () => {
    const page = await browser.newPage();
    const res = await page.goto(`http://localhost:${port}/api/health`);
    expect(res?.status()).toBe(200);
    const signedOut = await page.goto(`http://localhost:${port}/`);
    expect(signedOut?.status()).toBe(200);
    await expect(page.getByRole('heading', { name: 'This tab isn’t signed in' })).toBeVisible();
  });
});
