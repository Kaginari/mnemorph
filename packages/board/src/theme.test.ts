// @vitest-environment jsdom
import { expect, it } from 'vitest';
import { applyScheme, resolve, storedScheme } from './theme.ts';
import { ago } from './time.ts';

it('resolves a theme choice', () => {
  expect(resolve('auto', true)).toBe('light');
  expect(resolve('auto', false)).toBe('dark');
  expect(resolve('dark', true)).toBe('dark');
  expect(resolve('light', false)).toBe('light');
});

it('keeps the scheme per browser, neon unless another is chosen', () => {
  window.localStorage.removeItem('mnemorph.scheme');
  expect(storedScheme()).toBe('neon');
  window.localStorage.setItem('mnemorph.scheme', 'lagoon');
  expect(storedScheme()).toBe('lagoon');
  applyScheme(storedScheme());
  expect(document.documentElement.dataset.scheme).toBe('lagoon');
  window.localStorage.setItem('mnemorph.scheme', 'pink');
  expect(storedScheme()).toBe('neon');
  window.localStorage.removeItem('mnemorph.scheme');
});

it('says how long ago, short', () => {
  const now = Date.parse('2026-10-30T12:00:00Z');
  expect(ago('2026-10-30T11:59:30Z', now)).toBe('now');
  expect(ago('2026-10-30T11:55:00Z', now)).toBe('5m');
  expect(ago('2026-10-30T10:00:00Z', now)).toBe('2h');
  expect(ago('2026-10-27T12:00:00Z', now)).toBe('3d');
  expect(ago('2026-07-01T16:20:00Z', now)).toBe('2026-07-01');
  expect(ago('2026-10-30T13:00:00Z', now)).toBe('now');
  expect(ago('not a date', now)).toBe('unknown');
});
