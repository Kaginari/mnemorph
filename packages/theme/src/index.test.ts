import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { contrast, dark, light, type Palette, races } from './index.js';

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

describe('every text colour reads on every surface (WCAG AA, 4.5:1)', () => {
  for (const [name, p] of [
    ['dark', dark],
    ['light', light],
  ] as const) {
    for (const fg of ['text', 'textMuted', 'holo', 'alarm', 'gold'] as const) {
      for (const bg of ['bg', 'panel', 'panelRaised'] as const) {
        it(`${name}: ${fg} on ${bg}`, () =>
          expect(contrast(p[fg], p[bg])).toBeGreaterThanOrEqual(4.5));
      }
    }
  }
});
