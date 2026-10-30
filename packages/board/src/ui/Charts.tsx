/**
 * The board's charts, hand-made SVG over the tested data layer (`data/charts.ts`): stacked bars,
 * a line, a sparkline and a point with its interval. Colours are tokens, set by class; every series
 * is named in a legend beside it, never colour alone; a log axis says it is one. Words a world
 * wrote reach the SVG only as escaped text (`shown`), in `<title>` nodes React writes as text.
 */
import { line, linear, type Scale, short, stacked, whisker } from '../data/charts.ts';
import { Safe } from '../safe/Safe.tsx';
import { shown } from '../safe/shown.ts';
import { RACES } from './Mark.tsx';

const RACE_NAMES: Record<string, string> = Object.fromEntries(RACES);

/** A series' slot for its colour: a known race's own, else the neutral one. */
export function seriesOf(name: string): string {
  return name in RACE_NAMES ? name : 'other';
}

/** The legend: each series' swatch with its name. */
export function Legend({ series }: { series: readonly string[] }) {
  return (
    <ul className="chart-legend">
      {series.map((s) => (
        <li key={s} className="chart-key">
          <svg className="chart-swatch" viewBox="0 0 10 10" aria-hidden="true">
            <rect width="10" height="10" rx="2" className="chart-fill" data-series={seriesOf(s)} />
          </svg>
          <Safe text={RACE_NAMES[s] ?? s} />
        </li>
      ))}
    </ul>
  );
}

/** The y axis: its ticks, and "log" when it is one. */
function Axis({ y, width }: { y: Scale; width: number }) {
  return (
    <g className="chart-axis">
      {y.ticks.map((t) => (
        <g key={t.value}>
          <line x1={0} x2={width} y1={t.at} y2={t.at} className="chart-grid" />
          <text x={width + 4} y={t.at} dy="0.32em" className="chart-tick">
            {t.label}
          </text>
        </g>
      ))}
      {y.kind === 'log' && (
        <text x={0} y={-4} className="chart-tick chart-log">
          log axis
        </text>
      )}
    </g>
  );
}

const W = 320;
const H = 120;
const PAD = { top: 12, right: 40, bottom: 18 };

/** Spend per day, stacked by race. */
export function StackedBars({
  days,
  label,
  value,
}: {
  days: readonly { day: string; parts: readonly { series: string; value: number }[] }[];
  label: string;
  value: (v: number) => string;
}) {
  const b = stacked(
    days.map((d) => ({ key: d.day, parts: d.parts })),
    W,
    H,
  );
  return (
    <figure className="chart">
      <svg
        className="chart-svg"
        viewBox={`0 ${-PAD.top} ${W + PAD.right} ${H + PAD.top + PAD.bottom}`}
        role="img"
        aria-label={`${label}${b.y.kind === 'log' ? ', on a log axis' : ''}`}
      >
        <Axis y={b.y} width={W} />
        {b.bars.map((bar) => (
          <g key={bar.key}>
            {bar.segments.map((s) => (
              <rect
                key={s.series}
                x={bar.x}
                y={s.y}
                width={bar.w}
                height={Math.max(0, s.h)}
                className="chart-fill"
                data-series={seriesOf(s.series)}
              >
                <title>{shown(`${bar.key} · ${s.series}: ${value(s.value)}`)}</title>
              </rect>
            ))}
            {b.bars.length <= 10 && (
              <text x={bar.x + bar.w / 2} y={H + 13} textAnchor="middle" className="chart-tick">
                {bar.key.slice(5)}
              </text>
            )}
          </g>
        ))}
      </svg>
      <Legend series={b.series} />
    </figure>
  );
}

/** A share over time, 0–100%: the cache hit rate per day. */
export function ShareLine({
  values,
  keys,
  label,
}: {
  values: readonly (number | null)[];
  keys: readonly string[];
  label: string;
}) {
  const y = linear(0, 1, [H, 0], (v) => `${Math.round(v * 100)}%`, 2);
  const l = line(values, W, H, y);
  return (
    <figure className="chart">
      <svg
        className="chart-svg"
        viewBox={`0 ${-PAD.top} ${W + PAD.right} ${H + PAD.top + PAD.bottom}`}
        role="img"
        aria-label={label}
      >
        <Axis y={y} width={W} />
        <path d={l.path} className="chart-line" />
        {l.points.map((p, i) => (
          <circle key={keys[i] ?? i} cx={p.x} cy={p.y} r={3} className="chart-dot">
            <title>{shown(`${keys[i] ?? ''}: ${Math.round(p.value * 100)}%`)}</title>
          </circle>
        ))}
      </svg>
    </figure>
  );
}

/** A sparkline: weighted tokens per slot, no axis but its high mark. */
export function Sparkline({ values, label }: { values: readonly number[]; label: string }) {
  const h = 32;
  const w = 160;
  const l = line(values, w, h);
  const top = l.y.ticks.at(-1);
  return (
    <svg className="sparkline" viewBox={`0 -2 ${w + 34} ${h + 4}`} role="img" aria-label={label}>
      <path d={l.path} className="chart-line" />
      {top && (
        <text x={w + 4} y={6} className="chart-tick">
          {short(top.value)}
          {l.y.kind === 'log' ? ' log' : ''}
        </text>
      )}
    </svg>
  );
}

/** A meter: its point and its 95% interval on 0–1, or nothing to draw. */
export function Whisker({
  value,
  lo,
  hi,
  label,
}: {
  value: number | null;
  lo: number | null;
  hi: number | null;
  label: string;
}) {
  const w = 120;
  const s = linear(0, 1, [4, w - 4]);
  const at = whisker(value, lo, hi, s);
  return (
    <svg className="whisker" viewBox={`0 0 ${w} 14`} role="img" aria-label={label}>
      <line x1={4} x2={w - 4} y1={7} y2={7} className="chart-grid" />
      {at && (
        <>
          <line x1={at.from} x2={at.to} y1={7} y2={7} className="chart-whisker" />
          <line x1={at.from} x2={at.from} y1={3} y2={11} className="chart-whisker" />
          <line x1={at.to} x2={at.to} y1={3} y2={11} className="chart-whisker" />
          <circle cx={at.at} cy={7} r={3.5} className="chart-dot" />
        </>
      )}
    </svg>
  );
}
