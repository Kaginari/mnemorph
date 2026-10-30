/**
 * The charts' data layer: scales, ticks and the geometry of each chart, as plain numbers the SVG
 * components draw. No chart library: about four kinds, tested here on a single point and on a
 * wide range. A range wider than a thousandfold is drawn on a log axis, and the axis says so.
 */

export interface Tick {
  /** The value it marks. */
  value: number;
  /** Where, in the range. */
  at: number;
  /** Its words. */
  label: string;
}

export interface Scale {
  kind: 'linear' | 'log';
  /** The values it maps, low to high. */
  domain: [number, number];
  /** Where they go: `range[0]` for the low end. */
  range: [number, number];
  /** Where `v` goes; a value under the domain goes to its low end. */
  at: (v: number) => number;
  ticks: Tick[];
}

/** Past this ratio between the largest and the smallest positive value, an axis is log. */
export const LOG_PAST = 1000;

/** A count, short: 950, 1.2k, 3.4M. */
export function short(n: number): string {
  const a = Math.abs(n);
  if (a >= 1e9) return `${trim(n / 1e9)}G`;
  if (a >= 1e6) return `${trim(n / 1e6)}M`;
  if (a >= 1e3) return `${trim(n / 1e3)}k`;
  return trim(n);
}

function trim(n: number): string {
  const s = Math.abs(n) >= 100 || Number.isInteger(n) ? n.toFixed(0) : n.toFixed(1);
  return s.replace(/\.0$/, '');
}

/** A step of 1, 2 or 5 times a power of ten, near `span / count`. */
function step(span: number, count: number): number {
  const raw = span / Math.max(1, count);
  const p = 10 ** Math.floor(Math.log10(raw));
  const m = raw / p;
  return (m < 1.5 ? 1 : m < 3.5 ? 2 : m < 7.5 ? 5 : 10) * p;
}

/**
 * A linear scale over `[lo, hi]`. A single value — `lo === hi` — is given room: from zero to it,
 * or around it when it is zero or below.
 */
export function linear(
  lo: number,
  hi: number,
  range: [number, number],
  label: (v: number) => string = short,
  count = 4,
): Scale {
  let [a, b] = [Math.min(lo, hi), Math.max(lo, hi)];
  if (!Number.isFinite(a) || !Number.isFinite(b)) [a, b] = [0, 1];
  if (a === b) [a, b] = a > 0 ? [0, a] : a < 0 ? [a, 0] : [0, 1];
  const s = step(b - a, count);
  const ticks: Tick[] = [];
  const at = (v: number) =>
    range[0] + ((Math.max(a, Math.min(b, v)) - a) / (b - a)) * (range[1] - range[0]);
  for (let v = Math.ceil(a / s) * s; v <= b + s * 1e-9; v += s) {
    const value = Math.abs(v) < s * 1e-9 ? 0 : v;
    ticks.push({ value, at: at(value), label: label(value) });
  }
  return { kind: 'linear', domain: [a, b], range, at, ticks };
}

/**
 * A log scale over the positive values `[lo, hi]`, ticked at each power of ten; zero (or less)
 * goes to the low end.
 */
export function log(
  lo: number,
  hi: number,
  range: [number, number],
  label: (v: number) => string = short,
): Scale {
  const a = 10 ** Math.floor(Math.log10(Math.max(lo, Number.MIN_VALUE)));
  const b = 10 ** Math.ceil(Math.log10(Math.max(hi, a * 10)));
  const [la, lb] = [Math.log10(a), Math.log10(b)];
  const at = (v: number) =>
    v <= a
      ? range[0]
      : range[0] + ((Math.min(Math.log10(v), lb) - la) / (lb - la)) * (range[1] - range[0]);
  const ticks: Tick[] = [];
  const every = Math.max(1, Math.ceil((lb - la) / 7));
  for (let e = la; e <= lb; e += every) {
    const value = 10 ** e;
    ticks.push({ value, at: at(value), label: label(value) });
  }
  return { kind: 'log', domain: [a, b], range, at, ticks };
}

/**
 * The scale for `values` over `range`, from zero: linear, or log when the positive values span
 * more than [`LOG_PAST`]-fold.
 */
export function scaleFor(
  values: readonly number[],
  range: [number, number],
  label: (v: number) => string = short,
): Scale {
  const pos = values.filter((v) => Number.isFinite(v) && v > 0);
  if (pos.length === 0) return linear(0, 0, range, label);
  const max = Math.max(...pos);
  const min = Math.min(...pos);
  if (max / min > LOG_PAST) return log(min, max, range, label);
  return linear(0, max, range, label);
}

/** A part of a stacked bar. */
export interface Segment {
  series: string;
  value: number;
  /** Its top and height, in the chart's coordinates (y grows down). */
  y: number;
  h: number;
}

export interface Bar {
  key: string;
  total: number;
  x: number;
  w: number;
  segments: Segment[];
}

export interface Bars {
  bars: Bar[];
  y: Scale;
  /** Every series, in the order first met: each needs a label in the legend. */
  series: string[];
}

/**
 * Stacked bars, one per group, in a box `width` × `height` (y grows down). The total sets the
 * bar's height on the scale; on a log axis each part takes its share of that height, so the parts
 * still add up to the bar.
 */
export function stacked(
  groups: readonly { key: string; parts: readonly { series: string; value: number }[] }[],
  width: number,
  height: number,
): Bars {
  const totals = groups.map((g) => g.parts.reduce((s, p) => s + Math.max(0, p.value), 0));
  const y = scaleFor(totals, [height, 0]);
  const series: string[] = [];
  const slot = groups.length === 0 ? width : width / groups.length;
  const w = Math.max(1, Math.min(48, slot * 0.7));
  const bars = groups.map((g, i) => {
    const total = totals[i] ?? 0;
    const top = y.at(total);
    const full = height - top;
    let cursor = height;
    const segments: Segment[] = [];
    for (const p of g.parts) {
      if (!series.includes(p.series)) series.push(p.series);
      const v = Math.max(0, p.value);
      const h = total > 0 ? (v / total) * full : 0;
      cursor -= h;
      segments.push({ series: p.series, value: v, y: cursor, h });
    }
    return { key: g.key, total, x: slot * i + (slot - w) / 2, w, segments };
  });
  return { bars, y, series };
}

export interface Point {
  x: number;
  y: number;
  value: number;
}

/**
 * A line through `values` (`null` is a gap) in a box `width` × `height`, on `y` — or on a scale
 * from zero to the largest value. One point sits in the middle; the path's pieces break at gaps.
 */
export function line(
  values: readonly (number | null)[],
  width: number,
  height: number,
  y: Scale = scaleFor(
    values.filter((v): v is number => v !== null),
    [height, 0],
  ),
): { points: Point[]; path: string; y: Scale } {
  const n = values.length;
  const x = (i: number) => (n <= 1 ? width / 2 : (i / (n - 1)) * width);
  const points: Point[] = [];
  let path = '';
  let open = false;
  values.forEach((v, i) => {
    if (v === null || !Number.isFinite(v)) {
      open = false;
      return;
    }
    const p = { x: x(i), y: y.at(v), value: v };
    points.push(p);
    path += `${open ? 'L' : 'M'}${p.x.toFixed(1)} ${p.y.toFixed(1)}`;
    open = true;
  });
  return { points, path, y };
}

/** A point with its interval: where each falls on `scale`. */
export function whisker(
  value: number | null,
  lo: number | null,
  hi: number | null,
  scale: Scale,
): { at: number; from: number; to: number } | null {
  if (value === null || !Number.isFinite(value)) return null;
  const at = scale.at(value);
  return {
    at,
    from: scale.at(lo ?? value),
    to: scale.at(hi ?? value),
  };
}
