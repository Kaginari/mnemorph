/**
 * The chat, on the built page under the board server's CSP, against a fake bridge: the
 * transcript in the TUI's grammar (screenshots at phone and desktop, both themes, contrast and
 * labels), hostile text inert, a reconnect that replays without duplicating, an observer that
 * cannot send, a waiting ask shown with no approve control, Esc, the queue, a past run read-only
 * and resumed, and 2,000 messages scrolled in a window.
 */
import { fileURLToPath } from 'node:url';
import { expect, type Page, test } from '@playwright/test';
import { Bridge, CHAT, hello, LIVE, openChat, PAST, RUN, richTurn, violations } from './fake.ts';

const screen = (file: string) =>
  fileURLToPath(new URL(`../test-results/screens/${file}.png`, import.meta.url));

/** Text nodes below WCAG AA on what is behind them (as board.spec.ts measures). */
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
      if (!fg || fg[3] === 0) continue;
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

function unlabelled(page: Page) {
  return page.$$eval('[data-tone], [data-race]', (els) =>
    els
      .filter((e) => !(e.querySelector('.mark-label')?.textContent ?? '').trim())
      .map((e) => `${e.tagName} ${e.className}`),
  );
}

const transcript = (page: Page) => page.getByRole('region', { name: 'Transcript' });

for (const theme of ['dark', 'light'] as const) {
  test.describe(`${theme} theme`, () => {
    test.use({ colorScheme: theme });

    test('a chat reads in the TUI grammar: screenshot, contrast, labels', async ({
      page,
    }, info) => {
      const b = new Bridge();
      richTurn(b);
      b.emit('turnStarted', { text: 'and the docs?' });
      b.emit('toolCall', {
        id: 't9',
        name: 'bash',
        input: { command: 'pnpm --filter site build' },
      });
      await openChat(page, b);
      await expect(page.locator('.tool-name', { hasText: 'edit' })).toBeVisible();
      await expect(page.locator('.verdict')).toContainText('end gate · pass');
      await expect(page.locator('.group .tool-sum')).toContainText('read 1 file · grep 1 pattern');
      await expect(page.locator('.diff-add .diff-text')).toHaveText(
        '    return self._parse_collate(this)',
      );
      await expect(page.locator('.diff-add .diff-n')).toHaveText('2');
      await expect(page.locator('.line-below').first()).toContainText(
        'orc-sql is working on: judge',
      );
      await expect(page.locator('.activity')).toContainText('Running bash…');
      await expect(page.locator('.activity')).toContainText('esc to stop');
      // A replayed verdict is a line, never a toast.
      await expect(page.locator('.toast')).toHaveCount(0);
      if (info.project.name === 'desktop') {
        await expect(page.locator('#pane')).toContainText('41% / 100k');
        await expect(page.locator('#pane')).toContainText('$0.04');
        await expect(page.locator('#pane .file')).toContainText('parser.py');
        await expect(page.locator('#pane')).toContainText('orc-sql · done');
        await expect(page.locator('#pane')).toContainText('grounded');
        await expect(page.locator('.rail-chat').first()).toContainText('fix the collate parse');
      }
      await page.screenshot({ path: screen(`${info.project.name}-${theme}-chat`) });
      expect(await lowContrast(page)).toEqual([]);
      expect(await unlabelled(page)).toEqual([]);
      expect(await violations(page)).toEqual([]);
    });

    test('a waiting ask: the banner, and no approve control', async ({ page }, info) => {
      const b = new Bridge();
      b.emit('turnStarted', { text: 'tidy the wire' });
      await openChat(page, b);
      await expect(page.locator('.msg-user')).toHaveCount(1);
      b.say({
        jsonrpc: '2.0',
        id: 'gate-1',
        method: 'gate/ask',
        params: {
          body: 'orc-wire',
          race: 'orc',
          class: 'write',
          act: "sed -i 's/a/b/' crates/wire/src/lib.rs",
          why: ['writes a file in its zone'],
          outside: [],
          leftMs: 581_000,
        },
      });
      const banner = page.locator('.ask-banner');
      await expect(banner).toContainText('Waiting for your approval');
      await expect(banner).toContainText("sed -i 's/a/b/' crates/wire/src/lib.rs");
      await expect(banner).toContainText('approval card');
      await expect(banner).toContainText('9:41');
      await expect(banner.getByRole('button')).toHaveCount(0);
      await expect(page.getByRole('button', { name: /approve|allow|^yes/i })).toHaveCount(0);
      await expect(page.locator('.activity')).toContainText('Waiting for approval…');
      await page.screenshot({ path: screen(`${info.project.name}-${theme}-ask`) });
      expect(await lowContrast(page)).toEqual([]);
      // The page answered nothing: the ask waits.
      await page.waitForTimeout(300);
      expect(b.sent.filter((m) => m.id === 'gate-1')).toEqual([]);
    });
  });
}

test('hostile text from a model or a tool stays text', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const evil =
    '<img src=x onerror="window.__pwned=1"><script>window.__pwned=2</script> ‮gnp.exe \u001b[31mred\u001b[0m';
  const b = new Bridge();
  b.emit('turnStarted', { text: evil });
  b.emit('assistant', {
    text: `${evil}\n\n[click](javascript:window.__pwned=3) [data](data:text/html,<b>x</b>) ![beacon](https://evil.example/b.png) <a href="javascript:window.__pwned=4">a</a>\n\n\`\`\`html\n<script>window.__pwned=5</script>\n\`\`\``,
    stop: 'end_turn',
  });
  b.emit('toolCall', { id: 't1', name: 'bash', input: { command: '<b onmouseover=x>ls</b>' } });
  b.emit('toolResult', {
    id: 't1',
    content: `${evil}\nline2\nline3\nline4`,
    isError: false,
    cut: 0,
  });
  b.emit('note', { text: evil });
  b.emit('turnEnded', { ending: 'answered' });
  await openChat(page, b);
  await expect(page.locator('.msg-body')).toContainText('<img src=x onerror=');
  await expect(page.locator('.msg-body')).toContainText('(link not followed)');
  await expect(page.locator('.msg-body')).toContainText('image not loaded');
  await expect(page.locator('.msg-body .safe-esc').first()).toHaveText('\\u{202e}');
  await expect(page.locator('.out-text .safe-esc').first()).toBeVisible();
  const centre = page.locator('main');
  expect(await centre.locator('img, script, iframe, b, [onerror], [onmouseover]').count()).toBe(0);
  expect(await centre.locator('a[href^="javascript"], a[href^="data"]').count()).toBe(0);
  await page.locator('.out .fold').click();
  await expect(page.locator('.out-text')).toContainText('line4');
  expect(await page.evaluate(() => (window as { __pwned?: number }).__pwned)).toBeUndefined();
  expect(errors).toEqual([]);
  expect(await violations(page)).toEqual([]);
});

test('a dropped socket reconnects and replays, and nothing shows twice', async ({ page }) => {
  const b = new Bridge();
  b.emit('turnStarted', { text: 'first' });
  b.emit('assistant', { text: 'one', stop: 'end_turn' });
  b.emit('turnEnded', { ending: 'answered' });
  await openChat(page, b);
  await expect(page.locator('.msg-user')).toHaveCount(1);
  // The socket drops; while it is down the chat goes on.
  for (const s of b.sockets) await s.close({ code: 4000, reason: 'drop' });
  b.sockets = [];
  b.emit('turnStarted', { text: 'second' });
  b.emit('assistant', { text: 'two', stop: 'end_turn' });
  b.emit('turnEnded', { ending: 'answered' });
  await expect(page.locator('.msg-user')).toHaveCount(2);
  const since = b.sent.filter((m) => m.method === 'session/events').map((m) => m.params?.since);
  expect(since.at(-1)).toBe(3);
  // A live event the replay already carried arrives again: it is dropped by its seq.
  b.say(b.events[4]);
  b.emit('turnStarted', { text: 'third' });
  await expect(page.locator('.msg-user')).toHaveCount(3);
  await expect(page.locator('.msg-body', { hasText: 'two' })).toHaveCount(1);
  await expect(page.locator('.msg-user', { hasText: 'second' })).toHaveCount(1);
  // A reload replays from the start: still once each.
  await page.reload();
  await expect(page.locator('.msg-user')).toHaveCount(3);
  await expect(page.locator('.msg-body', { hasText: 'two' })).toHaveCount(1);
});

test('an observer cannot send; taking over lets it', async ({ page }) => {
  const b = new Bridge();
  b.role = 'observer';
  await openChat(page, b);
  const box = page.getByRole('textbox');
  await expect(page.locator('.observer')).toContainText('This tab observes');
  await expect(box).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Send' })).toBeDisabled();
  expect(b.sent.filter((m) => m.method === 'session/prompt')).toEqual([]);
  await page.getByRole('button', { name: 'Take over' }).click();
  await expect(box).toBeEnabled();
  await box.fill('hello');
  await box.press('Enter');
  await expect(page.locator('.msg-body')).toContainText('Answered: hello');
  expect(b.sent.filter((m) => m.method === 'board/takeOver')).toHaveLength(1);
});

test('while a turn runs, enter queues, Steer steers and Esc interrupts', async ({ page }) => {
  const b = new Bridge();
  b.onPrompt = (text) => b.emit('queued', { text });
  b.emit('turnStarted', { text: 'long job' });
  b.emit('toolCall', { id: 't1', name: 'bash', input: { command: 'sleep 60' } });
  await openChat(page, b);
  await expect(page.locator('.activity')).toContainText('Running bash…');
  const box = page.getByRole('textbox');
  await box.fill('next');
  await box.press('Enter');
  await expect(page.locator('.queued')).toContainText('queued: next');
  await box.fill('look at the lexer too');
  await page.getByRole('button', { name: 'Steer' }).click();
  await page.keyboard.press('Escape');
  await expect.poll(() => b.sent.map((m) => m.method)).toContain('session/interrupt');
  const sends = b.sent.filter((m) => m.method === 'session/send');
  expect(sends.map((m) => m.params?.text)).toEqual(['look at the lexer too']);
  b.emit('turnEnded', { ending: 'interrupted' });
  await expect(page.locator('.line-end')).toContainText('interrupted');
  // A verdict arriving live is a toast too.
  b.emit('gateVerdict', {
    passed: false,
    verdict: 'FAIL',
    changed: ['a.rs'],
    failures: ['cargo test'],
    unverified: [],
  });
  await expect(page.locator('.toast')).toContainText('end gate FAIL · 1 changed · 1 failed');
});

test('a past run reads from its journal, read-only, and resumes', async ({ page }) => {
  const b = new Bridge();
  const transcript = {
    run: PAST,
    state: 'stopped',
    dropped: 0,
    skipped: [],
    events: [
      {
        jsonrpc: '2.0',
        method: 'event/turnStarted',
        params: { session: '', run: PAST, body: '', seq: 1, text: 'wire refactor' },
      },
      {
        jsonrpc: '2.0',
        method: 'event/assistant',
        params: { session: '', run: PAST, body: '', seq: 2, text: 'Half done.', stop: 'end_turn' },
      },
      {
        jsonrpc: '2.0',
        method: 'event/turnEnded',
        params: { session: '', run: PAST, body: '', seq: 3, ending: 'interrupted' },
      },
    ],
  };
  let resumed: unknown;
  await openChat(page, b, `#/w/${LIVE.id}/r/${PAST}`, async (p) => {
    await p.route(`**/api/w/${LIVE.id}/runs/${PAST}`, (r) =>
      r.fulfill({ contentType: 'application/json', body: JSON.stringify(transcript) }),
    );
    await p.route(`**/api/w/${LIVE.id}/sessions`, async (r) => {
      resumed = r.request().postDataJSON();
      expect(r.request().headers()['x-mnemorph-csrf']).toBe('a'.repeat(64));
      await r.fulfill({
        status: 201,
        contentType: 'application/json',
        body: JSON.stringify({
          session: {
            id: CHAT,
            world: LIVE.id,
            session: 's-1',
            run: PAST,
            controlled: true,
            tabs: 0,
            asks: 0,
          },
          hello: { ...hello, run: PAST },
        }),
      });
    });
  });
  await expect(page.locator('.msg-user')).toContainText('wire refactor');
  await expect(page.locator('.line-end')).toContainText('interrupted');
  await expect(page.locator('.composer-closed')).toContainText('read-only');
  await expect(page.getByRole('textbox')).toHaveCount(0);
  await page.getByRole('button', { name: 'Resume' }).click();
  await expect(page).toHaveURL(new RegExp(`#/w/${LIVE.id}/s/${CHAT}$`));
  expect(resumed).toEqual({ resume: PAST });
  await expect(page.getByRole('textbox')).toBeEnabled();
});

test('2,000 messages scroll in a window, smoothly', async ({ page }, info) => {
  test.skip(info.project.name !== 'desktop', 'one measure is enough');
  const b = new Bridge();
  for (let i = 0; i < 1000; i++) {
    b.emit('turnStarted', { text: `question ${i}` });
    b.emit('assistant', {
      text: `Answer ${i}: ${'a few words of prose to wrap '.repeat((i % 5) + 1)}`,
      stop: 'end_turn',
    });
    b.emit('turnEnded', { ending: 'answered' });
  }
  await openChat(page, b);
  const list = transcript(page);
  await expect(list).toHaveAttribute('data-total', '2000');
  await expect(page.locator('.msg-body').last()).toContainText('Answer 999');
  // Only a window of rows is in the DOM.
  const rows = Number(await list.getAttribute('data-rows'));
  expect(rows).toBeGreaterThan(0);
  expect(rows).toBeLessThan(120);
  // Scrolled from the end to the start in steps, frame by frame: no frame stalls.
  const frames = await list.evaluate(async (el) => {
    const times: number[] = [];
    let last = performance.now();
    const steps = 60;
    const from = el.scrollTop;
    for (let i = 1; i <= steps; i++) {
      el.scrollTop = from * (1 - i / steps);
      await new Promise((r) => requestAnimationFrame(() => r(null)));
      const now = performance.now();
      times.push(now - last);
      last = now;
    }
    // Let measurement settle at the top.
    for (let i = 0; i < 10; i++) {
      el.scrollTop = 0;
      await new Promise((r) => requestAnimationFrame(() => r(null)));
    }
    return times.sort((a, b) => a - b);
  });
  const p90 = frames[Math.floor(frames.length * 0.9)] ?? Infinity;
  expect(p90, `frame times ${frames.map((t) => t.toFixed(0)).join(',')}`).toBeLessThan(50);
  expect(Math.max(...frames)).toBeLessThan(250);
  await expect(page.locator('.msg-user', { hasText: /^youquestion 0$/ })).toBeVisible();
  expect(Number(await list.getAttribute('data-rows'))).toBeLessThan(120);
  expect(RUN).toBeTruthy();
});
