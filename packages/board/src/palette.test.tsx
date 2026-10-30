/**
 * The colour-vision audit. Colour never carries meaning alone on the board: a world's state is a
 * glyph and a word as well as a tone, and a race always has its name beside its colour. These
 * tests hold that, and hold the one pairing the board does lean on by colour — present against
 * missing — apart under every deficiency.
 */
import { dark, light, races } from '@mnemorph/theme';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { apart, VISIONS } from './palette.ts';
import { RACES, RaceChip, STATUS } from './ui/Mark.tsx';

describe('a world state is more than its colour', () => {
  const all = Object.values(STATUS);
  it('each has its own glyph', () =>
    expect(new Set(all.map((s) => s.glyph)).size).toBe(all.length));
  it('each has its own words', () =>
    expect(new Set(all.map((s) => s.label)).size).toBe(all.length));
});

describe('present and missing (holo and alarm) stay apart under every colour-vision deficiency', () => {
  for (const [name, p] of [
    ['dark', dark],
    ['light', light],
  ] as const) {
    for (const v of VISIONS) {
      it(`${name}, ${v}: holo against alarm`, () =>
        expect(apart(p.holo, p.alarm, v)).toBeGreaterThanOrEqual(0.1));
    }
  }
});

describe('the races', () => {
  it('collide under a deficiency, which is why each is always named', () => {
    const vals = Object.values(races);
    const min = Math.min(
      ...VISIONS.flatMap((v) =>
        vals.flatMap((a, i) => vals.slice(i + 1).map((b) => apart(a, b, v))),
      ),
    );
    // High Orc, Oni and Veldora are three ambers: near-identical to anyone, by design of the art.
    expect(min).toBeLessThan(0.05);
  });
  it('every chip carries its race name', () => {
    for (const [race, label] of RACES) {
      const out = renderToStaticMarkup(<RaceChip race={race} name={label} />);
      expect(out.replace(/<[^>]+>/g, '')).toBe(label);
    }
  });
  it('every race token has a chip', () => {
    expect(RACES).toHaveLength(Object.keys(races).length);
  });
});
