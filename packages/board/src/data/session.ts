/**
 * The key `mnemorphd board` prints in the URL's fragment (`#k=<key>`) is traded once for the
 * session cookie: read, dropped from the address bar and history before anything renders, then
 * posted to `POST /session`. A page opened without a key relies on the cookie it already holds.
 *
 * The trade answers the CSRF token bound to the session. It lives in this module's memory only —
 * never storage, the DOM or a log — and a reloaded page asks for it again at `GET /api/session`
 * while the cookie lives. Every mutation sends it in `x-mnemorph-csrf`; a chat's WebSocket offers
 * it as the subprotocol entry `mnemorph.csrf.<token>` (the SDK's `ws({csrf})`).
 */

/** The key a fragment carries: 64 hex characters, or nothing. */
export function keyOf(hash: string): string | undefined {
  const m = /^#k=([0-9a-f]{64})$/.exec(hash);
  return m?.[1];
}

let pending: Promise<void> | undefined;
let token: string | undefined;

/**
 * Takes the key from the address bar at once (so it is never rendered, routed or kept in history)
 * and starts its trade. Called before the first render.
 */
export function takeKey() {
  const hash = window.location.hash;
  if (!hash.startsWith('#k=')) return;
  window.history.replaceState(null, '', `${window.location.pathname}${window.location.search}#/`);
  const key = keyOf(hash);
  if (!key) return;
  pending = fetch('/session', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ key }),
  }).then(async (res) => {
    if (!res.ok)
      throw new Error(
        `the board refused the key (${res.status}): it may be spent; open the URL “mnemorphd board” prints again`,
      );
    token = readToken(await res.json().catch(() => undefined));
  });
}

/** The token in a `{csrf}` answer, if it is one. */
function readToken(body: unknown): string | undefined {
  const t = (body as { csrf?: unknown } | undefined)?.csrf;
  return typeof t === 'string' && /^[0-9a-f]{64}$/.test(t) ? t : undefined;
}

/** The session's CSRF token: the one the trade answered, else asked of `GET /api/session`. */
export async function csrf(): Promise<string> {
  await establish().catch(() => {});
  if (token) return token;
  const res = await fetch('/api/session', { credentials: 'same-origin' });
  if (!res.ok)
    throw new Error(`no board session (${res.status}): open the URL “mnemorphd board” prints`);
  token = readToken(await res.json());
  if (!token) throw new Error('the board answered no CSRF token');
  return token;
}

/** A mutation: `POST path` with a JSON body and the CSRF token. */
export async function post(path: string, body: unknown): Promise<Response> {
  const t = await csrf();
  return fetch(path, {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'content-type': 'application/json', 'x-mnemorph-csrf': t },
    body: JSON.stringify(body),
  });
}

/** Forget the token: the session ended. */
export function forget(): void {
  token = undefined;
}

/** Settles once the key, if the page had one, has been traded. */
export function establish(): Promise<void> {
  return pending ?? Promise.resolve();
}
