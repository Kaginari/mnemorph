// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { csrf, establish, forget, keyOf, post, takeKey } from './session.ts';

const KEY = 'ab'.repeat(32);

afterEach(() => {
  vi.unstubAllGlobals();
  forget();
  window.history.replaceState(null, '', '/');
});

it('reads only a well-formed key', () => {
  expect(keyOf(`#k=${KEY}`)).toBe(KEY);
  expect(keyOf('#k=xyz')).toBeUndefined();
  expect(keyOf(`#k=${KEY}&x=1`)).toBeUndefined();
  expect(keyOf('#/w/a')).toBeUndefined();
});

it('drops the key from the address bar at once and posts it once', async () => {
  const fetch = vi.fn().mockResolvedValue({ ok: true, status: 200, json: async () => ({}) });
  vi.stubGlobal('fetch', fetch);
  window.history.replaceState(null, '', `/#k=${KEY}`);
  takeKey();
  expect(window.location.hash).toBe('#/');
  await establish();
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(fetch.mock.calls[0]?.[0]).toBe('/session');
  expect(JSON.parse(fetch.mock.calls[0]?.[1].body)).toEqual({ key: KEY });
});

it('drops a malformed key without sending it', () => {
  const fetch = vi.fn();
  vi.stubGlobal('fetch', fetch);
  window.history.replaceState(null, '', '/#k=<script>');
  takeKey();
  expect(window.location.hash).toBe('#/');
  expect(fetch).not.toHaveBeenCalled();
});

const TOKEN = 'cd'.repeat(32);

it('keeps the traded CSRF token in memory and sends it on a mutation', async () => {
  const fetch = vi
    .fn()
    .mockResolvedValue({ ok: true, status: 201, json: async () => ({ csrf: TOKEN }) });
  vi.stubGlobal('fetch', fetch);
  window.history.replaceState(null, '', `/#k=${KEY}`);
  takeKey();
  expect(await csrf()).toBe(TOKEN);
  await post('/api/w/w1/sessions', {});
  expect(fetch).toHaveBeenCalledTimes(2);
  expect(fetch.mock.calls[1]?.[1].headers['x-mnemorph-csrf']).toBe(TOKEN);
  // Nowhere but memory.
  expect(JSON.stringify({ ...window.localStorage })).not.toContain(TOKEN);
  expect(JSON.stringify({ ...window.sessionStorage })).not.toContain(TOKEN);
  expect(document.documentElement.textContent ?? '').not.toContain(TOKEN);
  expect(
    [...document.querySelectorAll('*')].some((e) =>
      [...e.attributes].some((a) => a.value.includes(TOKEN)),
    ),
  ).toBe(false);
});

it('a reloaded page asks for the token again while its cookie lives', async () => {
  const fetch = vi
    .fn()
    .mockResolvedValue({ ok: true, status: 200, json: async () => ({ csrf: TOKEN }) });
  vi.stubGlobal('fetch', fetch);
  expect(await csrf()).toBe(TOKEN);
  expect(fetch.mock.calls[0]?.[0]).toBe('/api/session');
  expect(await csrf()).toBe(TOKEN);
  expect(fetch).toHaveBeenCalledTimes(1);
  forget();
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 401 }));
  await expect(csrf()).rejects.toThrow(/no board session/);
});
