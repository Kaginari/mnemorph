/**
 * The pieces a world's overview is said in, on the home rows and the overview page alike: an
 * unknown reads "unknown" with its reason, never a zero; a spend with no price is never $0; every
 * model- or world-written word goes through `<Safe>`.
 */
import type { BoardOverview, Gate, Spend } from '@mnemorph/protocol';
import { Safe } from '../safe/Safe.tsx';
import { ago } from '../time.ts';
import { Mark, type Tone, WORLD_STATUS } from '../ui/Mark.tsx';

/** "unknown", with why on hover and for a screen reader. */
export function Unknown({ why }: { why: string | null }) {
  return (
    <span className="unknown" title={why ?? 'unknown'}>
      unknown
      {why && (
        <span className="visually-hidden">
          : <Safe text={why} />
        </span>
      )}
    </span>
  );
}

/** The world's status mark, with its words. */
export function StatusMark({ o }: { o: BoardOverview }) {
  if (o.status.value === null) {
    return (
      <Mark tone="muted" glyph="?">
        <Unknown why={o.status.why} />
      </Mark>
    );
  }
  const s = WORLD_STATUS[o.status.value];
  return (
    <Mark tone={s.tone} glyph={s.glyph}>
      {s.label}
    </Mark>
  );
}

/** How a verdict reads: a pass, a failure, anything else as it was written. */
export function verdictTone(verdict: string): Tone {
  if (verdict === 'PASS') return 'holo';
  if (verdict.startsWith('FAIL')) return 'alarm';
  return 'gold';
}

/** The gate's last verdict as a pill, or that it gave none. */
export function GatePill({ g }: { g: Gate }) {
  if (g.last === null) return <span>no gate verdict yet</span>;
  return (
    <Mark tone={verdictTone(g.last.verdict)} glyph="◆">
      gate <Safe text={g.last.verdict} />
    </Mark>
  );
}

/** Dollars, with what the price does not cover: never $0 for what was not priced. */
export function money(s: Spend): string {
  if (s.usd === null) return `unpriced (${s.unpriced} ${s.unpriced === 1 ? 'call' : 'calls'})`;
  const usd = `$${s.usd.toFixed(2)}`;
  return s.unpriced > 0 ? `${usd} + ${s.unpriced} unpriced` : usd;
}

/** When something happened, short, against `now`. */
export function When({ at, now }: { at: string; now: number }) {
  return <time dateTime={at}>{ago(at, now)}</time>;
}

/** Weighted tokens, short: 60.9k. */
export function tokens(wt: number): string {
  if (wt < 1000) return `${wt}`;
  if (wt < 1_000_000) return `${(wt / 1000).toFixed(1)}k`;
  return `${(wt / 1_000_000).toFixed(2)}M`;
}

/** A rate with its count and 95% interval, as `mnemorphd meters` says it; none, "no reports". */
export function rate(m: {
  successes: number | null;
  n: number | null;
  value: number | null;
  lo: number | null;
  hi: number | null;
}): string {
  if (m.n === null || m.successes === null || m.value === null) return 'no reports';
  const interval =
    m.lo !== null && m.hi !== null ? `, 95% ${m.lo.toFixed(2)}–${m.hi.toFixed(2)}` : '';
  return `${m.value.toFixed(2)} (${m.successes}/${m.n}${interval})`;
}
