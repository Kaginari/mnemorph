/**
 * The metrics, from `GET /api/metrics?world=<id|all>&window=7d|28d|all` (the board server's
 * `BoardMetrics`, generated from Rust): the usage ledgers' fold, the meters, the Dark Elf's scores
 * and the bench. Each part is checked before the page uses it; a part that does not hold together
 * reads as unknown, with why — never guessed at. Dollars never read $0 for what was not priced.
 */
import type {
  Bench,
  BenchRow,
  BoardMetrics,
  DayRaces,
  Meter,
  MetricsCreature,
  Named,
  Score,
  SessionSpent,
  Sourced,
  Spent,
  Usage,
} from '@mnemorph/protocol';
import { useCallback, useEffect, useState } from 'react';
import { type Change, usePush } from './push.ts';
import { establish } from './session.ts';

export const WINDOWS = ['7d', '28d', 'all'] as const;
export type Window = (typeof WINDOWS)[number];

type Obj = Record<string, unknown>;
const obj = (v: unknown): Obj | undefined =>
  typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Obj) : undefined;
const count = (v: unknown): v is number => typeof v === 'number' && Number.isInteger(v) && v >= 0;
const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const str = (v: unknown): v is string => typeof v === 'string';
const orNull = <T>(v: unknown, ok: (v: unknown) => v is T): v is T | null => v === null || ok(v);

/** Every element of `v` through `check`; `undefined` when `v` is no list or one does not hold. */
function list<T>(v: unknown, check: (v: unknown) => T | undefined): T[] | undefined {
  if (!Array.isArray(v)) return undefined;
  const out: T[] = [];
  for (const x of v) {
    const t = check(x);
    if (t === undefined) return undefined;
    out.push(t);
  }
  return out;
}

export function parseSpent(v: unknown): Spent | undefined {
  const o = obj(v);
  if (!o) return undefined;
  const { calls, wt, input, output, cacheRead, cacheWrite, usd, unpriced, cacheHit } = o;
  if (![calls, wt, input, output, cacheRead, cacheWrite, unpriced].every(count)) return undefined;
  if (!orNull(usd, num) || !orNull(cacheHit, num)) return undefined;
  return {
    calls: calls as number,
    wt: wt as number,
    input: input as number,
    output: output as number,
    cacheRead: cacheRead as number,
    cacheWrite: cacheWrite as number,
    usd,
    unpriced: unpriced as number,
    cacheHit,
  };
}

const named = (v: unknown): Named | undefined => {
  const o = obj(v);
  const spent = parseSpent(o?.spent);
  return o && str(o.name) && spent ? { name: o.name, spent } : undefined;
};

const day = (v: unknown): DayRaces | undefined => {
  const o = obj(v);
  const spent = parseSpent(o?.spent);
  const races = list(o?.races, named);
  return o && str(o.day) && spent && races ? { day: o.day, spent, races } : undefined;
};

const session = (v: unknown): SessionSpent | undefined => {
  const o = obj(v);
  const spent = parseSpent(o?.spent);
  if (!o || !spent || !str(o.session) || !str(o.world) || !str(o.first) || !str(o.last)) {
    return undefined;
  }
  return { session: o.session, world: o.world, first: o.first, last: o.last, spent };
};

function usage(v: unknown): Usage | undefined {
  const o = obj(v);
  if (!o) return undefined;
  const total = parseSpent(o.total);
  const days = list(o.days, day);
  const races = list(o.races, named);
  const models = list(o.models, named);
  const bodies = list(o.bodies, named);
  const sessions = list(o.sessions, session);
  const burn = list(o.burn, (x) => (count(x) ? x : undefined));
  if (!total || !days || !races || !models || !bodies || !sessions || !burn) return undefined;
  if (!count(o.burnSlotSecs) || !num(o.burnPerMinute)) return undefined;
  return {
    total,
    days,
    races,
    models,
    bodies,
    sessions,
    burn,
    burnSlotSecs: o.burnSlotSecs,
    burnPerMinute: o.burnPerMinute,
  };
}

export function parseMeter(v: unknown): Meter | undefined {
  const r = obj(v);
  if (!r || !str(r.meter)) return undefined;
  const { successes, n, value, lo, hi } = r;
  if (!orNull(successes, count) || !orNull(n, count)) return undefined;
  if (!orNull(value, num) || !orNull(lo, num) || !orNull(hi, num)) return undefined;
  return { meter: r.meter, successes, n, value, lo, hi };
}

const creature = (v: unknown): MetricsCreature | undefined => {
  const o = obj(v);
  const meters = list(o?.meters, parseMeter);
  if (!o || !meters || !str(o.world) || !str(o.creature) || !str(o.race)) return undefined;
  if (!orNull(o.firstCall, str)) return undefined;
  return { world: o.world, creature: o.creature, race: o.race, meters, firstCall: o.firstCall };
};

const score = (v: unknown): Score | undefined => {
  const o = obj(v);
  if (!o || !str(o.world) || !str(o.speciality) || !orNull(o.form, str)) return undefined;
  if (!orNull(o.proposal, str)) return undefined;
  const counts = ['findings', 'upheld', 'refuted', 'cannotVerify', 'held', 'unverified'] as const;
  if (!counts.every((k) => count(o[k]))) return undefined;
  const precision = parseMeter(o.precision);
  const upheldRate = parseMeter(o.upheldRate);
  const recent = parseMeter(o.recent);
  const cannotVerifyRate = parseMeter(o.cannotVerifyRate);
  if (!precision || !upheldRate || !recent || !cannotVerifyRate) return undefined;
  return {
    world: o.world,
    speciality: o.speciality,
    form: o.form,
    findings: o.findings as number,
    upheld: o.upheld as number,
    refuted: o.refuted as number,
    cannotVerify: o.cannotVerify as number,
    held: o.held as number,
    unverified: o.unverified as number,
    precision,
    upheldRate,
    recent,
    cannotVerifyRate,
    proposal: o.proposal,
  };
};

const benchRow = (v: unknown): BenchRow | undefined => {
  const o = obj(v);
  if (!o) return undefined;
  const { suite, harness, model, mode, valid, passed, invalid, usd, unpriced, date } = o;
  if (![suite, harness, model, mode].every(str)) return undefined;
  if (![valid, passed, invalid, unpriced].every(count)) return undefined;
  if (!orNull(usd, num) || !orNull(date, str)) return undefined;
  return {
    suite: suite as string,
    harness: harness as string,
    model: model as string,
    mode: mode as string,
    valid: valid as number,
    passed: passed as number,
    invalid: invalid as number,
    usd,
    unpriced: unpriced as number,
    date,
  };
};

const bench = (v: unknown): Bench | undefined => {
  const o = obj(v);
  const rows = list(o?.rows, benchRow);
  return o && rows && typeof o.configured === 'boolean' && count(o.unreadable)
    ? { configured: o.configured, rows, unreadable: o.unreadable }
    : undefined;
};

/** A part with its source; a value that does not read is unknown, and says so. */
export function sourced<T>(v: unknown, check: (v: unknown) => T | undefined): Sourced<T> {
  const o = obj(v);
  const source = o && str(o.source) ? o.source : 'unknown';
  const trusted = o?.trusted === true;
  const why = o && str(o.why) ? o.why : null;
  if (!o || o.value === null || o.value === undefined) {
    return { value: null, source, trusted, why: why ?? 'unknown' };
  }
  const value = check(o.value);
  if (value === undefined) {
    return { value: null, source, trusted, why: 'this part of the answer did not read' };
  }
  return { value, source, trusted, why };
}

/** The metrics an answer holds; `undefined` when it is not metrics at all. */
export function parseMetrics(body: unknown): BoardMetrics | undefined {
  const o = obj(body);
  const worlds = list(o?.worlds, (x) => (str(x) ? x : undefined));
  if (!o || !str(o.world) || !str(o.window) || !worlds) return undefined;
  return {
    world: o.world,
    window: o.window,
    worlds,
    usage: sourced(o.usage, usage),
    meters: sourced(o.meters, (x) => list(x, creature)),
    scores: sourced(o.scores, (x) => list(x, score)),
    bench: sourced(o.bench, bench),
  };
}

/** Dollars, never $0 for what was not priced: `$3.12`, `$3.12 (2 unpriced)`, `unpriced (2)`. */
export function dollars(usd: number | null, unpriced: number): string {
  if (usd === null) return unpriced > 0 ? `unpriced (${unpriced})` : 'no calls';
  const s = `$${usd.toFixed(2)}`;
  return unpriced > 0 ? `${s} (${unpriced} unpriced)` : s;
}

/** Weighted tokens, short: `1.9M wt`. */
export function wt(n: number): string {
  if (n < 1000) return `${n} wt`;
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k wt`;
  return `${(n / 1_000_000).toFixed(1)}M wt`;
}

/** The totals line: `$3.12 (2 unpriced) · 1.9M wt`. */
export function totals(s: Spent): string {
  return `${dollars(s.usd, s.unpriced)} · ${wt(s.wt)}`;
}

/** A share as a percentage, or a dash when there is none. */
export function pct(v: number | null): string {
  return v === null ? '—' : `${Math.round(v * 100)}%`;
}

/** What a meter says when it has no report: silent since the creature's first call, or never. */
export function silent(firstCall: string | null): string {
  return firstCall === null
    ? 'silent — no call booked yet'
    : `silent since ${firstCall.slice(0, 10)}`;
}

export type MetricsLoad =
  | { state: 'loading' }
  | { state: 'ready'; data: BoardMetrics }
  | { state: 'failed'; reason: string };

/** The metrics of `world` (or `all`) over `window`, read again when the push channel says. */
export function useMetrics(world: string, window: Window): MetricsLoad {
  const [load, setLoad] = useState<MetricsLoad>({ state: 'loading' });
  const [tick, setTick] = useState(0);
  const on = useCallback(
    (c: Change) => {
      if (c.kind === 'worlds' || world === 'all' || c.id === world) setTick((t) => t + 1);
    },
    [world],
  );
  usePush(on);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `tick` asks for a reread
  useEffect(() => {
    const ctl = new AbortController();
    const q = `world=${encodeURIComponent(world)}&window=${window}`;
    establish()
      .then(() =>
        fetch(`/api/metrics?${q}`, {
          signal: ctl.signal,
          credentials: 'same-origin',
          headers: { accept: 'application/json' },
        }),
      )
      .then(async (res) => {
        if (res.status === 401) throw new Error('this browser holds no board session');
        if (res.status === 404) throw new Error('the board lists no world with this id');
        if (!res.ok) throw new Error(`the board server answered ${res.status}`);
        const m = parseMetrics(await res.json());
        if (!m) throw new Error('the answer is not metrics');
        setLoad({ state: 'ready', data: m });
      })
      .catch((e: unknown) => {
        if (ctl.signal.aborted) return;
        setLoad({ state: 'failed', reason: e instanceof Error ? e.message : String(e) });
      });
    return () => ctl.abort();
  }, [world, window, tick]);
  return load;
}
