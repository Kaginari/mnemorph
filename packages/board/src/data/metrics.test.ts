import { expect, it } from 'vitest';
import { duration, tokens } from './metrics.ts';

it('says raw tokens short', () => {
  expect(tokens(950)).toBe('950');
  expect(tokens(12_345)).toBe('12.3k');
  expect(tokens(4_100_000)).toBe('4.1M');
});

it('says a duration short', () => {
  expect(duration(0.42)).toBe('0.4s');
  expect(duration(42)).toBe('42s');
  expect(duration(184)).toBe('3m 04s');
  expect(duration(4320)).toBe('1h 12m');
});
