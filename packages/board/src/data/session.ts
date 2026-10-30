/**
 * The key `mnemorphd board` prints in the URL's fragment (`#k=<key>`) is traded once for the
 * session cookie: read, dropped from the address bar and history before anything renders, then
 * posted to `POST /session`. A page opened without a key relies on the cookie it already holds.
 */

/** The key a fragment carries: 64 hex characters, or nothing. */
export function keyOf(hash: string): string | undefined {
  const m = /^#k=([0-9a-f]{64})$/.exec(hash);
  return m?.[1];
}

let pending: Promise<void> | undefined;

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
  }).then((res) => {
    if (!res.ok)
      throw new Error(
        `the board refused the key (${res.status}): it may be spent; open the URL “mnemorphd board” prints again`,
      );
  });
}

/** Settles once the key, if the page had one, has been traded. */
export function establish(): Promise<void> {
  return pending ?? Promise.resolve();
}
