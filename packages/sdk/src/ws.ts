/**
 * The WebSocket transport: a tab on a board chat, `ws://127.0.0.1:<port>/ws/session/<id>`. It
 * offers the subprotocol `mnemorph.v1` and the CSRF token as `mnemorph.csrf.<token>` (a browser's
 * WebSocket cannot set a header), and reconnects after a drop with a growing pause. On each
 * reopening the `Client` takes control back if it had it and replays what it missed by `seq`.
 * A close that says the chat is over — ended, signed out, unknown, crowded, refused — is final.
 */
import type { Transport } from './client.js';

/** The subprotocol a chat's socket speaks. */
export const SUBPROTOCOL = 'mnemorph.v1';

/** The close codes after which reconnecting would not help. */
export const FINAL_CLOSES: ReadonlySet<number> = new Set([
  1003, 1008, 1009, 4001, 4401, 4404, 4429,
]);

/** The part of the browser's `WebSocket` the transport uses. */
export interface SocketLike {
  readonly readyState: number;
  send(data: string): void;
  close(code?: number, reason?: string): void;
  onopen: ((ev: unknown) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onclose: ((ev: { code: number; reason: string }) => void) | null;
  onerror: ((ev: unknown) => void) | null;
}

/** How a socket is made: the browser's `WebSocket`, or a test's. */
export type SocketMaker = (url: string, protocols: string[]) => SocketLike;

export interface WsOptions {
  /** The chat's socket: `ws://127.0.0.1:<port>/ws/session/<id>`. */
  url: string;
  /** The board session's CSRF token, asked for at each connection. Keep it in memory only:
   * never in storage, the DOM or a log. */
  csrf: () => string | Promise<string>;
  /** Makes the socket; the global `WebSocket` by default. */
  socket?: SocketMaker;
  /** The first pause before reconnecting, in ms; it doubles up to `maxDelay`. */
  delay?: number;
  maxDelay?: number;
  /** Failed tries in a row before giving up. */
  attempts?: number;
}

const OPEN = 1;

/** Speak to a board chat over a WebSocket, reconnecting after a drop. */
export function ws(options: WsOptions): Transport {
  const make: SocketMaker =
    options.socket ?? ((url, protocols) => new WebSocket(url, protocols) as unknown as SocketLike);
  const first = options.delay ?? 250;
  const most = options.maxDelay ?? 5000;
  const attempts = options.attempts ?? 20;
  const lines: ((line: string) => void)[] = [];
  const closers: ((why: string) => void)[] = [];
  const drops: ((why: string) => void)[] = [];
  const reopens: (() => void)[] = [];
  let sock: SocketLike | undefined;
  /** What is sent before the first connection opens, sent once it does. */
  let early: string[] | undefined = [];
  let opened = false;
  let failures = 0;
  let over = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const finish = (why: string) => {
    if (over) return;
    over = true;
    if (timer) clearTimeout(timer);
    for (const c of closers) c(why);
  };

  const connect = async () => {
    let token: string;
    try {
      token = await options.csrf();
    } catch (e) {
      return finish(`no CSRF token: ${(e as Error).message}`);
    }
    if (over) return;
    const s = make(options.url, [SUBPROTOCOL, `mnemorph.csrf.${token}`]);
    sock = s;
    let up = false;
    s.onopen = () => {
      up = true;
      failures = 0;
      if (opened) for (const r of reopens) r();
      opened = true;
      for (const line of early ?? []) s.send(line);
      early = undefined;
    };
    s.onmessage = (ev) => {
      if (typeof ev.data === 'string') for (const l of lines) l(ev.data);
    };
    s.onerror = () => {};
    s.onclose = (ev) => {
      if (sock !== s || over) return;
      sock = undefined;
      const why = ev.reason || `the socket closed (${ev.code})`;
      if (FINAL_CLOSES.has(ev.code)) return finish(why);
      if (!up) failures += 1;
      if (failures >= attempts) return finish(`${why}; gave up after ${attempts} tries`);
      for (const d of drops) d(why);
      const pause = Math.min(most, first * 2 ** failures);
      timer = setTimeout(() => void connect(), pause);
    };
  };
  void connect();

  return {
    send: (line) => {
      if (early) early.push(line);
      else if (sock && sock.readyState === OPEN) sock.send(line);
    },
    onLine: (l) => lines.push(l),
    onClose: (l) => closers.push(l),
    onDrop: (l) => drops.push(l),
    onReopen: (l) => reopens.push(l),
    ready: () => early !== undefined || (sock !== undefined && sock.readyState === OPEN),
    close: () => {
      const s = sock;
      sock = undefined;
      finish('the client closed the connection');
      s?.close(1000, 'bye');
    },
  };
}
