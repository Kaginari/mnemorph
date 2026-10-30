/**
 * A board in the test: the API routed to fixtures, and each chat's WebSocket answered by a fake
 * bridge speaking the board's side of the protocol — `initialize`, `session/events` replay by
 * `seq`, `session/prompt|send|interrupt`, `board/takeOver`, roles and asks — so the page runs
 * against the protocol without a server.
 */
import type { Page, WebSocketRoute } from '@playwright/test';
import { FIXTURE_NOW, fixtureWorlds } from '../src/data/fixtures.ts';

export const LIVE =
  fixtureWorlds.worlds.find((w) => w.name === 'tempest-try') ??
  (() => {
    throw new Error('the fixtures hold tempest-try');
  })();

export const CHAT = 'c-7f3a';
export const RUN = 'r1793361000000-41';
export const PAST = 'r1793300000000-12';

type Msg = {
  id?: number | string;
  method?: string;
  params?: Record<string, unknown>;
  result?: unknown;
};

export const hello = {
  protocol: 1,
  version: '0.0.0',
  world: LIVE.path,
  model: 'deepseek-v4-flash',
  provider: 'openrouter',
  session: 's-1',
  run: RUN,
  holes: [],
  budget: null,
  key: null,
  sandbox: 'landlock',
  body: 'rimuru',
  held: 0,
};

/** An event of the session's own body, or of `body`. */
export function event(seq: number, type: string, fields: Record<string, unknown>, body = 'rimuru') {
  return {
    jsonrpc: '2.0',
    method: `event/${type}`,
    params: { session: 's-1', run: RUN, body, seq, ...fields },
  };
}

/** A fake bridge for one chat: its events, the tabs' sockets and what they sent. */
export class Bridge {
  events: ReturnType<typeof event>[] = [];
  sockets: WebSocketRoute[] = [];
  sent: Msg[] = [];
  role: 'controller' | 'observer' = 'controller';
  /** What a prompt does: by default a turn that answers in one line. */
  onPrompt: (text: string) => void = (text) => {
    this.emit('turnStarted', { text });
    this.emit('assistant', { text: `Answered: ${text}`, stop: 'end_turn' });
    this.emit('turnEnded', { ending: 'answered' });
  };

  /** Add an event, numbered next, and send it to every tab. */
  emit(type: string, fields: Record<string, unknown>, body = 'rimuru') {
    const e = event(this.events.length + 1, type, fields, body);
    this.events.push(e);
    for (const s of this.sockets) s.send(JSON.stringify(e));
  }

  say(msg: unknown) {
    for (const s of this.sockets) s.send(JSON.stringify(msg));
  }

  attach(ws: WebSocketRoute) {
    this.sockets.push(ws);
    ws.onClose(() => {
      this.sockets = this.sockets.filter((s) => s !== ws);
    });
    ws.send(
      JSON.stringify({
        jsonrpc: '2.0',
        method: 'board/role',
        params: { role: this.role, controlled: true },
      }),
    );
    ws.onMessage((raw) => {
      const m = JSON.parse(String(raw)) as Msg;
      this.sent.push(m);
      const answer = (result: unknown) =>
        ws.send(JSON.stringify({ jsonrpc: '2.0', id: m.id, result }));
      const refuse = (message: string) =>
        ws.send(JSON.stringify({ jsonrpc: '2.0', id: m.id, error: { code: -32011, message } }));
      switch (m.method) {
        case 'initialize':
          return answer(hello);
        case 'session/events': {
          const since = Number(m.params?.since ?? 0);
          return answer({
            events: this.events.filter((e) => e.params.seq > since),
            oldest: this.events.length ? 1 : null,
            complete: true,
            more: false,
          });
        }
        case 'board/takeOver':
          this.role = 'controller';
          answer({});
          return ws.send(
            JSON.stringify({
              jsonrpc: '2.0',
              method: 'board/role',
              params: { role: 'controller', controlled: true },
            }),
          );
        case 'session/prompt':
          if (this.role !== 'controller') return refuse('this tab observes: take over to act');
          answer({ queued: false });
          return this.onPrompt(String(m.params?.text ?? ''));
        case 'session/send':
        case 'session/interrupt':
          if (this.role !== 'controller') return refuse('this tab observes: take over to act');
          return answer({});
        default:
          return undefined;
      }
    });
  }
}

/** The runs a world lists. */
export const runs = {
  runs: [
    {
      id: RUN,
      when: '2026-10-30T11:58:30Z',
      state: 'live',
      why: 'cut mid-turn',
      ask: 'fix the collate parse',
      turns: 1,
    },
    {
      id: PAST,
      when: '2026-10-29T16:20:00Z',
      state: 'stopped',
      why: 'interrupted',
      ask: 'wire refactor',
      turns: 3,
    },
    {
      id: 'r1793200000000-3',
      when: '2026-10-28T09:00:00Z',
      state: 'ended',
      why: 'answered',
      ask: 'what does notes.txt say?',
      turns: 1,
    },
  ],
  more: 0,
};

/**
 * Opens the board at `hash` with the API routed and `bridge` answering the chat's socket. CSP
 * violations are collected, as the shell's suite does.
 */
export async function openChat(
  page: Page,
  bridge: Bridge,
  hash = `#/w/${LIVE.id}/s/${CHAT}`,
  extra?: (p: Page) => Promise<void>,
) {
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
  await page.route('**/api/session', (r) => r.fulfill(json({ csrf: 'a'.repeat(64) })));
  await page.route('**/api/sessions', (r) =>
    r.fulfill(
      json({
        sessions: [
          {
            id: CHAT,
            world: LIVE.id,
            session: 's-1',
            run: RUN,
            controlled: true,
            tabs: 1,
            asks: 0,
          },
        ],
      }),
    ),
  );
  await page.route(`**/api/w/${LIVE.id}/runs`, (r) => r.fulfill(json(runs)));
  await page.route(`**/api/w/${LIVE.id}`, (r) =>
    r.fulfill(
      json({
        memory: {
          value: { shared: 12, grounded: 41, proposed: 9, retired: 3, unproven: 0 },
          source: '.mnemorph/memory',
          trusted: false,
          why: null,
        },
      }),
    ),
  );
  await page.routeWebSocket('**/ws/board', () => {});
  await page.routeWebSocket(`**/ws/session/${CHAT}`, (ws) => bridge.attach(ws));
  if (extra) await extra(page);
  await page.goto(`/${hash}`);
}

export function violations(page: Page) {
  return page.evaluate(() => (window as { __csp?: string[] }).__csp ?? ['no CSP listener']);
}

/** A rich turn in the TUI's grammar: looks grouped, an edit's diff, folded output, a body below,
 * the gate. */
export function richTurn(b: Bridge) {
  b.emit('turnStarted', { text: 'fix the collate parse' });
  b.emit('assistant', {
    text: "I'll ask **orc-sql** to judge, then read the parser.",
    stop: 'tool_use',
  });
  b.emit('toolCall', {
    id: 't1',
    name: 'read',
    input: { path: 'sqlglot/parser.py', offset: 120, limit: 40 },
  });
  b.emit('toolResult', {
    id: 't1',
    content: '120\tdef _parse_types(self):',
    isError: false,
    cut: 0,
  });
  b.emit('toolCall', { id: 't2', name: 'grep', input: { pattern: 'COLLATE' } });
  b.emit('toolResult', { id: 't2', content: 'parser.py:131', isError: false, cut: 0 });
  b.emit('turnStarted', { text: 'judge the collate fix' }, 'orc-sql');
  b.emit(
    'toolCall',
    { id: 'o1', name: 'bash', input: { command: 'pytest -q tests/test_parser.py' } },
    'orc-sql',
  );
  b.emit('turnEnded', { ending: 'answered' }, 'orc-sql');
  b.emit('toolCall', {
    id: 't3',
    name: 'edit',
    input: {
      path: 'sqlglot/parser.py',
      old_string: '    this = self._parse_column()\n    return this\n',
      new_string: '    this = self._parse_column()\n    return self._parse_collate(this)\n',
    },
  });
  b.emit('toolResult', { id: 't3', content: 'edited', isError: false, cut: 0 });
  b.emit('toolCall', { id: 't4', name: 'bash', input: { command: 'pytest -q' } });
  b.emit('toolResult', {
    id: 't4',
    content: '........................\n........\n412 passed in 3.1s\nwarnings: 2\n',
    isError: false,
    cut: 0,
  });
  b.emit('usage', {
    input: 52_000,
    output: 1_900,
    cacheRead: 7_000,
    cacheWrite: 0,
    usd: 0.04,
    wt: 60_900,
    spentWt: 60_900,
    occupancy: 41_000,
    stressLine: 100_000,
  });
  b.emit('assistant', {
    text: 'Collations now survive `_parse_types`:\n\n```python\nreturn self._parse_collate(this)  # keep it\n```\n\n- 412 tests pass\n- one file changed',
    stop: 'end_turn',
  });
  b.emit('gateVerdict', {
    passed: true,
    verdict: 'PASS',
    changed: ['sqlglot/parser.py'],
    failures: [],
    unverified: [],
  });
  b.emit('turnEnded', { ending: 'answered' });
}
