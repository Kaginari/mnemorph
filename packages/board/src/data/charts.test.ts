import { describe, expect, it } from 'vitest';
import { LOG_PAST, line, linear, log, scaleFor, short, stacked, whisker } from './charts.ts';

const finite = (n: number) => Number.isFinite(n);

describe('scales', () => {
  it('give a single point room, never a zero-width domain', () => {
    for (const v of [0, 7, -3, 1e9]) {
      const s = linear(v, v, [100, 0]);
      expect(s.domain[0]).toBeLessThan(s.domain[1]);
      expect(finite(s.at(v))).toBe(true);
      expect(s.ticks.length).toBeGreaterThan(0);
    }
    expect(scaleFor([42], [100, 0]).at(42)).toBe(0);
    expect(scaleFor([], [100, 0]).kind).toBe('linear');
  });

  it('go log past a thousandfold, ticked and labelled at powers of ten', () => {
    const wide = [3, 40, 1_900_000];
    const s = scaleFor(wide, [100, 0]);
    expect(s.kind).toBe('log');
    expect(Math.max(...wide) / Math.min(...wide)).toBeGreaterThan(LOG_PAST);
    expect(s.ticks.map((t) => t.label)).toEqual([
      '1',
      '10',
      '100',
      '1k',
      '10k',
      '100k',
      '1M',
      '10M',
    ]);
    for (const v of wide) {
      const y = s.at(v);
      expect(y).toBeGreaterThanOrEqual(0);
      expect(y).toBeLessThanOrEqual(100);
    }
    expect(s.at(3)).toBeGreaterThan(s.at(40));
    expect(s.at(0)).toBe(100);
    expect(scaleFor([10, 9000], [100, 0]).kind).toBe('linear');
  });

  it('thin out the ticks of a very wide log axis', () => {
    expect(log(1, 1e18, [0, 100]).ticks.length).toBeLessThanOrEqual(7);
  });

  it('say counts short', () => {
    expect([950, 1200, 3_400_000, 12, 0.5].map(short)).toEqual([
      '950',
      '1.2k',
      '3.4M',
      '12',
      '0.5',
    ]);
  });
});

describe('stacked bars', () => {
  it('stack each day by race, the parts adding up to the bar', () => {
    const b = stacked(
      [
        {
          key: 'd1',
          parts: [
            { series: 'orc', value: 10 },
            { series: 'slime', value: 30 },
          ],
        },
        { key: 'd2', parts: [{ series: 'slime', value: 40 }] },
      ],
      200,
      100,
    );
    expect(b.series).toEqual(['orc', 'slime']);
    const [d1] = b.bars;
    expect(d1?.total).toBe(40);
    const h = d1?.segments.reduce((s, x) => s + x.h, 0) ?? 0;
    expect(h).toBeCloseTo(100);
    expect(d1?.segments[0]?.y).toBeCloseTo(75);
  });

  it('draw a single bar', () => {
    const b = stacked([{ key: 'only', parts: [{ series: 'orc', value: 5 }] }], 300, 100);
    expect(b.bars).toHaveLength(1);
    expect(b.bars[0]?.segments[0]?.h).toBeCloseTo(100);
    expect(b.bars[0]?.x).toBeGreaterThan(0);
  });

  it('keep a tiny day visible beside a huge one on a log axis', () => {
    const b = stacked(
      [
        { key: 'small', parts: [{ series: 'orc', value: 2 }] },
        { key: 'huge', parts: [{ series: 'orc', value: 5_000_000 }] },
      ],
      200,
      100,
    );
    expect(b.y.kind).toBe('log');
    expect(b.bars[0]?.segments[0]?.h ?? 0).toBeGreaterThan(4);
    for (const bar of b.bars)
      for (const s of bar.segments) expect(finite(s.y) && finite(s.h)).toBe(true);
  });

  it('draw nothing for no days', () => {
    expect(stacked([], 100, 100).bars).toEqual([]);
  });
});

describe('lines and whiskers', () => {
  it('put a single point in the middle', () => {
    const l = line([0.7], 200, 100, linear(0, 1, [100, 0]));
    expect(l.points).toHaveLength(1);
    expect(l.points[0]?.x).toBe(100);
    expect(l.points[0]?.y).toBeCloseTo(30);
    expect(l.path).toBe('M100.0 30.0');
  });

  it('break at a gap and stay finite on a wide range', () => {
    const l = line([1, null, 2_000_000, 3], 300, 50);
    expect(l.y.kind).toBe('log');
    expect(l.path.match(/M/g)).toHaveLength(2);
    for (const p of l.points) expect(finite(p.x) && finite(p.y)).toBe(true);
  });

  it('place a meter and its interval', () => {
    const s = linear(0, 1, [0, 100]);
    expect(whisker(0.9, 0.7, 0.97, s)).toEqual({ at: 90, from: 70, to: 97 });
    expect(whisker(null, null, null, s)).toBeNull();
    expect(whisker(1, null, null, s)).toEqual({ at: 100, from: 100, to: 100 });
  });
});
