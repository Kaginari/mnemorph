/**
 * A world's overview, from `GET /api/w/<id>` (the board server's `BoardOverview`, generated from
 * Rust), read again whenever the push channel says the world moved. Each field is checked before
 * the page uses it: a field that does not hold together reads as unknown — its value `null`, with
 * the reason — never guessed at.
 */
import type {
  Activity,
  BoardOverview,
  ChallengeCounts,
  CreatureMeters,
  Gate,
  Memory,
  Sourced,
  Spend,
  WorldStatus,
} from '@mnemorph/protocol';
import { useCallback, useEffect, useState } from 'react';
import { type Change, usePush } from './push.ts';
import { establish, signedOut } from './session.ts';

/** Every status a world can read. */
export const WORLD_STATUSES: readonly WorldStatus[] = ['live', 'tui-live', 'idle', 'missing'];

type Check<T> = (v: unknown) => T | undefined;

const obj = (v: unknown): Record<string, unknown> | undefined =>
  typeof v === 'object' && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : undefined;
const count = (v: unknown): v is number => typeof v === 'number' && Number.isInteger(v) && v >= 0;
const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const str = (v: unknown): v is string => typeof v === 'string';
const when = (v: unknown): v is string => str(v) && !Number.isNaN(Date.parse(v));
const orNull = <T>(v: unknown, ok: (v: unknown) => v is T): v is T | null => v === null || ok(v);

/** The counts `keys` names, all whole and not negative. */
function counts<T>(keys: readonly string[]): Check<T> {
  return (v) => {
    const o = obj(v);
    if (!o || !keys.every((k) => count(o[k]))) return undefined;
    return Object.fromEntries(keys.map((k) => [k, o[k]])) as T;
  };
}

const status: Check<WorldStatus> = (v) =>
  WORLD_STATUSES.includes(v as WorldStatus) ? (v as WorldStatus) : undefined;

const activity: Check<Activity> = (v) => {
  const o = obj(v);
  return o && when(o.at) && str(o.what) ? { at: o.at, what: o.what } : undefined;
};

const memory = counts<Memory>(['shared', 'grounded', 'proposed', 'retired', 'unproven']);

const challenges = counts<ChallengeCounts>([
  'pending',
  'blocked',
  'verifying',
  'unstaffed',
  'findings',
  'unreadable',
]);

const gate: Check<Gate> = (v) => {
  const o = obj(v);
  if (!o || !count(o.verdicts)) return undefined;
  if (o.last === null) return { verdicts: o.verdicts, last: null };
  const l = obj(o.last);
  if (!l || !when(l.at) || !str(l.body) || !str(l.verdict) || !count(l.changed)) return undefined;
  return {
    verdicts: o.verdicts,
    last: { at: l.at, body: l.body, verdict: l.verdict, changed: l.changed },
  };
};

const meters: Check<CreatureMeters[]> = (v) => {
  if (!Array.isArray(v)) return undefined;
  const out: CreatureMeters[] = [];
  for (const c of v) {
    const o = obj(c);
    if (!o || !str(o.creature) || !str(o.race) || !Array.isArray(o.meters)) return undefined;
    const ms = [];
    for (const m of o.meters) {
      const r = obj(m);
      if (!r || !str(r.meter)) return undefined;
      const { successes, n, value, lo, hi } = r;
      if (!orNull(successes, count) || !orNull(n, count)) return undefined;
      if (!orNull(value, num) || !orNull(lo, num) || !orNull(hi, num)) return undefined;
      ms.push({ meter: r.meter, successes, n, value, lo, hi });
    }
    out.push({ creature: o.creature, race: o.race, meters: ms });
  }
  return out;
};

const spend: Check<Spend> = (v) => {
  const o = obj(v);
  if (!o || !str(o.session) || !count(o.calls) || !count(o.unpriced) || !count(o.wt)) {
    return undefined;
  }
  if (!orNull(o.usd, num)) return undefined;
  return { session: o.session, calls: o.calls, usd: o.usd, unpriced: o.unpriced, wt: o.wt };
};

const waiting: Check<number> = (v) => (count(v) ? v : undefined);

const updates: Check<string> = (v) => (v === 'pushed' || v === 'slowed' ? v : undefined);

/** A field with its source; a value that does not read is unknown, and says so. */
function sourced<T>(v: unknown, check: Check<T>): Sourced<T> {
  const o = obj(v);
  const source = o && str(o.source) ? o.source : 'unknown';
  const trusted = o?.trusted === true;
  const why = o && str(o.why) ? o.why : null;
  if (!o || o.value === null || o.value === undefined) {
    return { value: null, source, trusted, why: why ?? 'unknown' };
  }
  const value = check(o.value);
  if (value === undefined) {
    return { value: null, source, trusted, why: 'the answer for this field did not read' };
  }
  return { value, source, trusted, why };
}

/** The overview an answer holds; `undefined` when it is not one at all. */
export function parseOverview(body: unknown): BoardOverview | undefined {
  const o = obj(body);
  const w = obj(o?.world);
  if (!o || !w || !str(w.id) || !str(w.name) || !str(w.path)) return undefined;
  return {
    world: o.world as BoardOverview['world'],
    status: sourced(o.status, status),
    activity: sourced(o.activity, activity),
    memory: sourced(o.memory, memory),
    challenges: sourced(o.challenges, challenges),
    gate: sourced(o.gate, gate),
    meters: sourced(o.meters, meters),
    spend: sourced(o.spend, spend),
    waiting: sourced(o.waiting, waiting),
    updates: sourced(o.updates, updates),
  };
}

export type OverviewLoad =
  | { state: 'loading' }
  | { state: 'ready'; data: BoardOverview; at: number }
  | { state: 'failed'; reason: string; data: BoardOverview | undefined };

/** `GET /api/w/<id>`, read. */
async function read(id: string, signal: AbortSignal): Promise<BoardOverview> {
  await establish();
  const res = await fetch(`/api/w/${encodeURIComponent(id)}`, {
    signal,
    credentials: 'same-origin',
    headers: { accept: 'application/json' },
  });
  if (res.status === 401) {
    signedOut();
    throw new Error('this browser holds no board session: open the URL `mnemorphd board` prints');
  }
  if (res.status === 404) throw new Error('the board lists no world with this id');
  if (!res.ok) throw new Error(`the board server answered ${res.status}`);
  const o = parseOverview(await res.json());
  if (!o) throw new Error('the answer is not an overview');
  return o;
}

/**
 * The overview of world `id`, read on mount and again each time the push channel names it (or
 * says everything may have moved). A failed reread keeps what was shown, said to be stale.
 */
export function useOverview(id: string): OverviewLoad {
  const [load, setLoad] = useState<OverviewLoad>({ state: 'loading' });
  const [tick, setTick] = useState(0);
  const on = useCallback(
    (c: Change) => {
      if (c.kind === 'worlds' || c.id === id) setTick((t) => t + 1);
    },
    [id],
  );
  usePush(on);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `tick` asks for a reread
  useEffect(() => {
    const ctl = new AbortController();
    read(id, ctl.signal)
      .then((data) => setLoad({ state: 'ready', data, at: Date.now() }))
      .catch((e: unknown) => {
        if (ctl.signal.aborted) return;
        const reason = e instanceof Error ? e.message : String(e);
        setLoad((was) => ({
          state: 'failed',
          reason,
          data: was.state === 'ready' ? was.data : was.state === 'failed' ? was.data : undefined,
        }));
      });
    return () => ctl.abort();
  }, [id, tick]);
  return load;
}
