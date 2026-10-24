import { contrast } from '@mnemorph/theme';
import { expect, test } from 'vitest';
import { chooseTheme, palette } from './theme.js';

test('the human word wins, then the terminal says', () => {
  expect(chooseTheme('light', { COLORFGBG: '15;0' })).toBe('light');
  expect(chooseTheme(undefined, { MNEMORPH_THEME: 'light' })).toBe('light');
  expect(chooseTheme(undefined, { COLORFGBG: '0;15' })).toBe('light');
  expect(chooseTheme(undefined, { COLORFGBG: '15;default;0' })).toBe('dark');
  expect(chooseTheme(undefined, { COLORFGBG: '0;7' })).toBe('light');
  expect(chooseTheme(undefined, {})).toBe('dark');
  expect(chooseTheme('auto', {})).toBe('dark');
});

test('each palette keeps its text readable on its own background', () => {
  for (const name of ['dark', 'light'] as const) {
    const p = palette(name);
    expect(contrast(p.text, p.bg)).toBeGreaterThan(7);
    expect(contrast(p.textMuted, p.bg)).toBeGreaterThan(4.5);
  }
});
