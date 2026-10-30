// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { establish, keyOf, takeKey } from './session.ts';

const KEY = 'ab'.repeat(32);

afterEach(() => {
  vi.unstubAllGlobals();
  window.history.replaceState(null, '', '/');
});

it('reads only a well-formed key', () => {
  expect(keyOf(`#k=${KEY}`)).toBe(KEY);
  expect(keyOf('#k=xyz')).toBeUndefined();
  expect(keyOf(`#k=${KEY}&x=1`)).toBeUndefined();
  expect(keyOf('#/w/a')).toBeUndefined();
});

it('drops the key from the address bar at once and posts it once', async () => {
  const fetch = vi.fn().mockResolvedValue({ ok: true, status: 200 });
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
