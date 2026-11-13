import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  contrast,
  DEFAULT_SCHEME,
  dark,
  light,
  type Palette,
  races,
  SCHEMES,
  type SchemePalette,
  schemes,
} from './index.js';

const css = readFileSync(new URL('./tokens.css', import.meta.url), 'utf8');

/** The `--mn-*` declarations of the first block opened by `selector`. */
function block(selector: string): Record<string, string> {
  const start = css.indexOf(`${selector} {`);
  const body = css.slice(start, css.indexOf('}', start));
  return Object.fromEntries(
    [...body.matchAll(/--mn-([a-z-]+):\s*([^;]+);/g)].map((m) => [m[1], (m[2] ?? '').trim()]),
  );
}

const kebab = (k: string) => k.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);

describe('the css and the ts agree', () => {
  it.each([
    [':root', dark],
    [':root[data-theme="light"]', light],
  ] as const)('%s', (selector, palette: Palette) => {
    const decl = block(selector);
    for (const [k, v] of Object.entries(palette)) expect(decl[kebab(k)], k).toBe(v);
  });

  it('the races', () => {
    const decl = block(':root');
    for (const [k, v] of Object.entries(races)) expect(decl[`race-${kebab(k)}`], k).toBe(v);
  });
});

describe('every scheme: the css and the ts agree', () => {
  for (const s of SCHEMES.filter((s) => s !== 'hud')) {
    it.each([
      [`:root[data-scheme="${s}"]:not([data-theme="light"])`, schemes[s].dark],
      [`:root[data-scheme="${s}"][data-theme="light"]`, schemes[s].light],
    ] as const)('%s', (selector, palette: SchemePalette) => {
      const decl = block(selector);
      for (const [k, v] of Object.entries(palette)) {
        const name = k === 'player' ? 'race-player' : kebab(k);
        expect(decl[name], k).toBe(v);
      }
    });
  }
  it('neon is the default, and every scheme is listed', () => {
    expect(DEFAULT_SCHEME).toBe('neon');
    expect(Object.keys(schemes).sort()).toEqual([...SCHEMES].sort());
  });
});

describe('every text colour reads on every surface (WCAG AA, 4.5:1)', () => {
  for (const [name, p] of [
    ['dark', dark],
    ['light', light],
    ...SCHEMES.flatMap((s) => [
      [`${s} dark`, schemes[s].dark],
      [`${s} light`, schemes[s].light],
    ]),
  ] as [string, Palette][]) {
    for (const fg of ['text', 'textMuted', 'holo', 'alarm', 'gold'] as const) {
      for (const bg of ['bg', 'panel', 'panelRaised'] as const) {
        it(`${name}: ${fg} on ${bg}`, () =>
          expect(contrast(p[fg], p[bg])).toBeGreaterThanOrEqual(4.5));
      }
    }
  }
});
