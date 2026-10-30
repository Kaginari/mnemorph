import { expect, test } from 'vitest';
import { type Arrived, Client, type GateReply, type SocketLike, ws } from './browser.js';

/** A board in memory: the sockets the transport opened, driven by the test. */
class Board {
  sockets: FakeSocket[] = [];
  make = (url: string, protocols: string[]) => {
    const s = new FakeSocket(url, protocols);
    this.sockets.push(s);
    return s;
  };
  get last(): FakeSocket {
    const s = this.sockets.at(-1);
    if (!s) throw new Error('no socket');
    return s;
  }
}

class FakeSocket implements SocketLike {
  readyState = 0;
  sent: Record<string, unknown>[] = [];
  onopen: ((ev: unknown) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: unknown) => void) | null = null;
  constructor(
    readonly url: string,
    readonly protocols: string[],
  ) {}
  send(data: string) {
    this.sent.push(JSON.parse(data));
  }
  close() {
    this.readyState = 3;
  }
  open() {
    this.readyState = 1;
    this.onopen?.({});
  }
  say(msg: unknown) {
    this.onmessage?.({ data: JSON.stringify(msg) });
  }
  /** The connection ends with `code`: 1006 is a drop. */
  drop(code = 1006, reason = '') {
    this.readyState = 3;
    this.onclose?.({ code, reason });
  }
  /** The last request sent with `method`. */
  request(method: string) {
    const r = this.sent.filter((m) => m.method === method).at(-1);
    if (!r) throw new Error(`no ${method} sent`);
    return r as { id: number; params: Record<string, unknown> };
  }
  answer(method: string, result: unknown) {
    this.say({ jsonrpc: '2.0', id: this.request(method).id, result });
  }
}

const event = (type: string, seq: number, fields: Record<string, unknown> = {}) => ({
  jsonrpc: '2.0',
  method: `event/${type}`,
  params: { session: 's', run: 'r', body: 'rimuru', seq, ...fields },
});

const role = (r: string) => ({
  jsonrpc: '2.0',
  method: 'board/role',
  params: { role: r, controlled: true },
});

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms));

function connect(board: Board, csrf = () => 'tok') {
  return new Client(
    ws({ url: 'ws://127.0.0.1:7878/ws/session/abc', csrf, socket: board.make, delay: 1 }),
  );
}

test('the socket offers the chat subprotocol and the CSRF token, fetched per connection', async () => {
  const board = new Board();
  let n = 0;
  const c = connect(board, () => `t${++n}`);
  await tick();
  expect(board.last.url).toBe('ws://127.0.0.1:7878/ws/session/abc');
  expect(board.last.protocols).toEqual(['mnemorph.v1', 'mnemorph.csrf.t1']);
  // A request before the socket opens waits for it.
  const hello = c.initialize({ name: 't' });
  board.last.open();
  board.last.answer('initialize', { protocol: 1, session: 's' });
  await hello;
  board.last.drop();
  await tick(5);
  expect(board.sockets).toHaveLength(2);
  expect(board.last.protocols).toEqual(['mnemorph.v1', 'mnemorph.csrf.t2']);
});

test('a socket killed mid-turn delivers every event once, in order, after it reconnects', async () => {
  const board = new Board();
  const c = connect(board);
  const seen: number[] = [];
  c.on((e: Arrived) => seen.push(e.seq));
  await tick();
  board.last.open();
  board.last.say(role('controller'));
  for (const s of [1, 2, 3]) board.last.say(event('textDelta', s, { text: `${s}` }));
  // A request in flight fails with the drop, never hangs.
  const waiting = c.prompt('more');
  board.last.drop();
  await expect(waiting).rejects.toThrow(/closed/);
  await tick(5);
  const s2 = board.last;
  s2.open();
  await tick();
  // Control is taken back, then the replay asks from the last seq seen.
  s2.answer('board/takeOver', {});
  await tick();
  expect(s2.request('session/events').params).toEqual({ since: 3 });
  // Live events arrive while the replay runs: they wait, and duplicates drop.
  s2.say(event('textDelta', 7));
  s2.say(event('textDelta', 6));
  s2.answer('session/events', {
    events: [event('textDelta', 3), event('textDelta', 4)],
    oldest: 1,
    complete: true,
    more: true,
  });
  await tick();
  expect(s2.request('session/events').params).toEqual({ since: 4 });
  s2.answer('session/events', {
    events: [event('textDelta', 5), event('textDelta', 6)],
    oldest: 1,
    complete: true,
    more: false,
  });
  await tick();
  s2.say(event('turnEnded', 8, { ending: 'answered' }));
  expect(seen).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
});

test('an ask sent again after a reload is asked once, and answered on the new socket', async () => {
  const board = new Board();
  const c = connect(board);
  let asked = 0;
  let decide: (r: GateReply) => void = () => {};
  c.onGate(() => {
    asked += 1;
    return new Promise<GateReply>((r) => {
      decide = r;
    });
  });
  await tick();
  board.last.open();
  board.last.say(role('controller'));
  const ask = {
    jsonrpc: '2.0',
    id: 'gate-1',
    method: 'gate/ask',
    params: { act: 'rm x', leftMs: 600000 },
  };
  board.last.say(ask);
  board.last.drop();
  await tick(5);
  const s2 = board.last;
  s2.open();
  s2.say(role('controller'));
  s2.say({ ...ask, params: { act: 'rm x', leftMs: 570000 } });
  expect(asked).toBe(1);
  expect(c.asks.get('gate-1')?.leftMs).toBe(570000);
  decide({ allow: true });
  await tick();
  expect(s2.sent.filter((m) => m.id === 'gate-1')).toEqual([
    { jsonrpc: '2.0', id: 'gate-1', result: { allow: true } },
  ]);
  expect(c.asks.size).toBe(0);
});

test('an observer answers nothing until it takes over', async () => {
  const board = new Board();
  const c = connect(board);
  let asked = 0;
  c.onGate(() => {
    asked += 1;
    return { allow: false, reason: 'no' };
  });
  const roles: string[] = [];
  c.onRole((r) => roles.push(r));
  await tick();
  board.last.open();
  board.last.say(role('observer'));
  board.last.say({ jsonrpc: '2.0', id: 'gate-2', method: 'gate/ask', params: { act: 'x' } });
  await tick();
  expect(asked).toBe(0);
  expect([...c.asks.keys()]).toEqual(['gate-2']);
  const taking = c.takeOver();
  board.last.answer('board/takeOver', {});
  await taking;
  board.last.say(role('controller'));
  board.last.say({ jsonrpc: '2.0', id: 'gate-2', method: 'gate/ask', params: { act: 'x' } });
  await tick();
  expect(asked).toBe(1);
  expect(roles).toEqual(['observer', 'controller']);
  // An ask withdrawn by the board leaves the list.
  board.last.say({ jsonrpc: '2.0', id: 'gate-3', method: 'gate/ask', params: { act: 'y' } });
  board.last.say({ jsonrpc: '2.0', method: 'board/askGone', params: { id: 'gate-3' } });
  await tick();
  expect(c.asks.has('gate-3')).toBe(false);
});

test('a close that ends the chat is final; other drops reconnect until the tries run out', async () => {
  const board = new Board();
  const c = connect(board);
  const notices: string[] = [];
  c.onNotice((n) => notices.push(n.method));
  await tick();
  board.last.open();
  board.last.say({ jsonrpc: '2.0', method: 'board/ended', params: { why: 'the board stopped' } });
  board.last.drop(4001, 'the chat ended');
  await tick(10);
  expect(board.sockets).toHaveLength(1);
  expect(c.closed).toBe('the chat ended');
  expect(notices).toEqual(['board/ended']);
  await expect(c.prompt('x')).rejects.toThrow('the chat ended');

  const again = new Board();
  const d = new Client(
    ws({ url: 'ws://x/ws/session/y', csrf: () => 't', socket: again.make, delay: 1, attempts: 3 }),
  );
  for (let i = 0; i < 3; i++) {
    await tick(10);
    again.last.drop();
  }
  await tick(10);
  expect(again.sockets).toHaveLength(3);
  expect(d.closed).toMatch(/gave up after 3 tries/);
});
