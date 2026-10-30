/**
 * A world's knowledge: its memory (`GET /api/w/<id>/memory`, `BoardMemory`) and its ontology
 * (`GET /api/w/<id>/facts`, `BoardFacts`), generated from Rust, read again whenever the push
 * channel names the world. Each answer is checked before the page uses it: a row that does not
 * hold together is left out and counted, never guessed at. The human's acts — confirm, wake,
 * settle — are POSTs with the CSRF token; the server checks each again and answers what was done
 * or why not.
 */
import type {
  BoardFact,
  BoardFactStatus,
  BoardFacts,
  BoardMemory,
  FactAnchor,
  FactFinding,
  FactSide,
  MemoryCreature,
  MemoryNote,
  NoteScope,
  NoteStanding,
  PathOwner,
  ProofVerdict,
} from '@mnemorph/protocol';
import { useCallback, useEffect, useState } from 'react';
import { type Change, usePush } from './push.ts';
import { establish, post } from './session.ts';

/** The statuses of a fact, in the order the page tabs them: the facts table's partition. */
export const FACT_STATUSES: readonly BoardFactStatus[] = [
  'contested',
  'single',
  'agreed',
  'settled',
  'countered',
];

export const NOTE_STANDINGS: readonly NoteStanding[] = [
  'proposed',
  'grounded',
  'retired',
  'dormant',
  'tombstoned',
];

export const NOTE_SCOPES: readonly NoteScope[] = ['personal', 'shared'];

const obj = (v: unknown): Record<string, unknown> | undefined =>
  typeof v === 'object' && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : undefined;
const count = (v: unknown): v is number => typeof v === 'number' && Number.isInteger(v) && v >= 0;
const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const str = (v: unknown): v is string => typeof v === 'string';
const strOrNull = (v: unknown): v is string | null => v === null || str(v);
const bool = (v: unknown): v is boolean => typeof v === 'boolean';
const strs = (v: unknown): v is string[] => Array.isArray(v) && v.every(str);
const hex64 = (v: unknown): v is string => str(v) && /^[0-9a-f]{64}$/.test(v);

/** The rows of `v` that `row` reads, and how many did not. */
function rows<T>(v: unknown, row: (r: unknown) => T | undefined): { ok: T[]; bad: number } {
  const ok: T[] = [];
  let bad = 0;
  for (const r of Array.isArray(v) ? v : []) {
    const t = row(r);
    if (t === undefined) bad += 1;
    else ok.push(t);
  }
  return { ok, bad };
}

function note(v: unknown): MemoryNote | undefined {
  const o = obj(v);
  if (!o) return undefined;
  const { id, creature, scope, standing, text, cut, route, proof, confidence, version } = o;
  if (!str(id) || !str(creature) || !str(text) || !bool(cut)) return undefined;
  if (!NOTE_SCOPES.includes(scope as NoteScope)) return undefined;
  if (!NOTE_STANDINGS.includes(standing as NoteStanding)) return undefined;
  if (!strOrNull(route) || !strOrNull(proof) || !num(confidence) || !count(version)) {
    return undefined;
  }
  if (!strOrNull(o.review) || !strs(o.groundings) || !num(o.born) || !hex64(o.seen)) {
    return undefined;
  }
  if (!bool(o.confirm) || !bool(o.wake)) return undefined;
  const versions = rows(o.versions, (r) => {
    const x = obj(r);
    return x && count(x.n) && num(x.time) && str(x.text)
      ? { n: x.n, time: x.time, text: x.text }
      : undefined;
  });
  const depends = rows(o.depends, (r) => {
    const x = obj(r);
    return x && str(x.on) && str(x.what) ? { on: x.on, what: x.what } : undefined;
  });
  if (versions.bad > 0 || depends.bad > 0) return undefined;
  return {
    id,
    creature,
    scope: scope as NoteScope,
    standing: standing as NoteStanding,
    text,
    cut,
    route,
    proof,
    confidence,
    version,
    versions: versions.ok,
    depends: depends.ok,
    review: o.review,
    groundings: o.groundings,
    born: o.born,
    seen: o.seen,
    confirm: o.confirm,
    wake: o.wake,
  };
}

function creature(v: unknown): MemoryCreature | undefined {
  const o = obj(v);
  if (!o || !str(o.creature) || !bool(o.frozen) || !strs(o.pathologies)) return undefined;
  if (!count(o.unproven) || !count(o.proposed) || !count(o.retired) || !count(o.review)) {
    return undefined;
  }
  const events = rows(o.events, (r) => {
    const x = obj(r);
    return x && str(x.file) && str(x.event) && str(x.note) && str(x.why)
      ? { file: x.file, event: x.event, note: x.note, why: x.why }
      : undefined;
  });
  if (events.bad > 0) return undefined;
  return {
    creature: o.creature,
    frozen: o.frozen,
    events: events.ok,
    unproven: o.unproven,
    pathologies: o.pathologies,
    proposed: o.proposed,
    retired: o.retired,
    review: o.review,
  };
}

export interface MemoryPage extends BoardMemory {
  /** Rows the answer held that did not read. */
  unreadable: number;
}

/** The memory an answer holds; `undefined` when it is not one at all. */
export function parseMemory(body: unknown): MemoryPage | undefined {
  const o = obj(body);
  const s = obj(o?.standing);
  if (!o || !s || !str(o.source)) return undefined;
  const keys = ['shared', 'grounded', 'proposed', 'retired', 'unproven'] as const;
  if (!keys.every((k) => count(s[k]))) return undefined;
  if (!count(o.total) || !count(o.offset) || !count(o.limit)) return undefined;
  const notes = rows(o.notes, note);
  const creatures = rows(o.creatures, creature);
  return {
    standing: Object.fromEntries(keys.map((k) => [k, s[k]])) as unknown as BoardMemory['standing'],
    creatures: creatures.ok,
    notes: notes.ok,
    total: o.total,
    offset: o.offset,
    limit: o.limit,
    source: o.source,
    unreadable: notes.bad + creatures.bad,
  };
}

function anchor(v: unknown): FactAnchor | null | undefined {
  if (v === null) return null;
  const o = obj(v);
  return o && str(o.at) && bool(o.stale) ? { at: o.at, stale: o.stale } : undefined;
}

function side(v: unknown): FactSide | undefined {
  const o = obj(v);
  if (!o || !str(o.creature) || !strOrNull(o.reason)) return undefined;
  const a = anchor(o.anchor);
  return a === undefined ? undefined : { creature: o.creature, anchor: a, reason: o.reason };
}

function owner(v: unknown): PathOwner | undefined {
  const o = obj(v);
  return o && str(o.path) && str(o.owner) && str(o.why)
    ? { path: o.path, owner: o.owner, why: o.why }
    : undefined;
}

function fact(v: unknown): BoardFact | undefined {
  const o = obj(v);
  if (!o || !str(o.id) || !str(o.about) || !str(o.text) || !hex64(o.seen)) return undefined;
  if (!FACT_STATUSES.includes(o.status as BoardFactStatus) || !strOrNull(o.settled)) {
    return undefined;
  }
  const supporters = rows(o.supporters, side);
  const counters = rows(o.counters, side);
  const a = anchor(o.anchor);
  const w = owner(o.owner);
  if (supporters.bad > 0 || counters.bad > 0 || a === undefined || !w) return undefined;
  return {
    id: o.id,
    about: o.about,
    text: o.text,
    status: o.status as BoardFactStatus,
    supporters: supporters.ok,
    counters: counters.ok,
    anchor: a,
    settled: o.settled,
    owner: w,
    seen: o.seen,
  };
}

function finding(v: unknown): FactFinding | undefined {
  const o = obj(v);
  return o && str(o.severity) && str(o.code) && str(o.at) && str(o.message)
    ? { severity: o.severity, code: o.code, at: o.at, message: o.message }
    : undefined;
}

function verdict(v: unknown): ProofVerdict | undefined {
  const o = obj(v);
  return o && count(o.n) && o.n > 0 && str(o.at) && str(o.body) && strs(o.changed)
    ? { n: o.n, at: o.at, body: o.body, changed: o.changed }
    : undefined;
}

export interface FactsPage extends BoardFacts {
  /** Rows the answer held that did not read. */
  unreadable: number;
}

/** The ontology an answer holds; `undefined` when it is not one at all. */
export function parseFacts(body: unknown): FactsPage | undefined {
  const o = obj(body);
  const c = obj(o?.counts);
  if (!o || !c || !FACT_STATUSES.every((k) => count(c[k]))) return undefined;
  if (!count(o.total) || !count(o.offset) || !count(o.limit) || !count(o.found)) return undefined;
  if (!bool(o.failed) || !bool(o.git) || !strs(o.problems) || !str(o.source)) return undefined;
  const facts = rows(o.facts, fact);
  const findings = rows(o.findings, finding);
  const owners = rows(o.owners, owner);
  const verdicts = rows(o.verdicts, verdict);
  return {
    counts: Object.fromEntries(
      FACT_STATUSES.map((k) => [k, c[k]]),
    ) as unknown as BoardFacts['counts'],
    facts: facts.ok,
    total: o.total,
    offset: o.offset,
    limit: o.limit,
    failed: o.failed,
    findings: findings.ok,
    found: o.found,
    problems: o.problems,
    owners: owners.ok,
    verdicts: verdicts.ok,
    git: o.git,
    source: o.source,
    unreadable: facts.bad + findings.bad + owners.bad + verdicts.bad,
  };
}

export type KnowledgeLoad<T> =
  | { state: 'loading' }
  | { state: 'ready'; data: T }
  | { state: 'failed'; reason: string; data: T | undefined };

/** The reason a refusal gives: its plain-text body, else its status. */
async function why(res: Response): Promise<string> {
  const text = await res.text().catch(() => '');
  return text.trim() === '' ? `the board server answered ${res.status}` : text.trim();
}

async function read<T>(
  path: string,
  parse: (b: unknown) => T | undefined,
  signal: AbortSignal,
): Promise<T> {
  await establish();
  const res = await fetch(path, {
    signal,
    credentials: 'same-origin',
    headers: { accept: 'application/json' },
  });
  if (res.status === 401) {
    throw new Error('this browser holds no board session: open the URL `mnemorphd board` prints');
  }
  if (!res.ok) throw new Error(await why(res));
  const v = parse(await res.json());
  if (v === undefined) throw new Error('the answer does not read');
  return v;
}

/**
 * `path` of world `id`, read on mount, when `query` changes, when `again` moves, and each time the
 * push channel names the world. A failed reread keeps what was shown.
 */
function useKnowledge<T>(
  id: string,
  path: string,
  parse: (b: unknown) => T | undefined,
  again: number,
): KnowledgeLoad<T> {
  const [load, setLoad] = useState<KnowledgeLoad<T>>({ state: 'loading' });
  const [tick, setTick] = useState(0);
  const on = useCallback(
    (c: Change) => {
      if (c.kind === 'worlds' || c.id === id) setTick((t) => t + 1);
    },
    [id],
  );
  usePush(on);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `tick` and `again` ask for a reread
  useEffect(() => {
    const ctl = new AbortController();
    read(path, parse, ctl.signal)
      .then((data) => setLoad({ state: 'ready', data }))
      .catch((e: unknown) => {
        if (ctl.signal.aborted) return;
        const reason = e instanceof Error ? e.message : String(e);
        setLoad((was) => ({
          state: 'failed',
          reason,
          data: was.state === 'loading' ? undefined : was.data,
        }));
      });
    return () => ctl.abort();
  }, [path, tick, again]);
  return load;
}

const base = (id: string) => `/api/w/${encodeURIComponent(id)}`;

/** What the memory page asks for. */
export interface MemoryQuery {
  creature?: string | undefined;
  scope?: NoteScope | undefined;
  standing?: NoteStanding | undefined;
  review?: boolean | undefined;
  offset?: number | undefined;
}

/** The query string a memory filter is. */
export function memoryQuery(q: MemoryQuery): string {
  const p = new URLSearchParams();
  if (q.creature) p.set('creature', q.creature);
  if (q.scope) p.set('scope', q.scope);
  if (q.standing) p.set('standing', q.standing);
  if (q.review !== undefined) p.set('review', q.review ? 'yes' : 'no');
  if (q.offset) p.set('offset', String(q.offset));
  const s = p.toString();
  return s === '' ? '' : `?${s}`;
}

export function useMemory(id: string, q: MemoryQuery, again: number) {
  return useKnowledge(id, `${base(id)}/memory${memoryQuery(q)}`, parseMemory, again);
}

export function useFacts(id: string, status: BoardFactStatus, offset: number, again: number) {
  const p = new URLSearchParams({ status });
  if (offset) p.set('offset', String(offset));
  return useKnowledge(id, `${base(id)}/facts?${p}`, parseFacts, again);
}

/** What an act answered: done, with the server's words, or refused, with its reason. */
export type Outcome = { ok: true; done: string } | { ok: false; why: string };

async function act(path: string, body: unknown): Promise<Outcome> {
  try {
    const res = await post(path, body);
    if (!res.ok) return { ok: false, why: await why(res) };
    const done = obj(await res.json().catch(() => undefined))?.done;
    return { ok: true, done: str(done) ? done : 'done' };
  } catch (e) {
    return { ok: false, why: e instanceof Error ? e.message : String(e) };
  }
}

/** The confirmed route on note `n`, as the human read it. */
export function confirmNote(world: string, n: MemoryNote): Promise<Outcome> {
  return act(`${base(world)}/memory/${encodeURIComponent(n.id)}/confirm`, { seen: n.seen });
}

/** A retired note `n` woken, as the human read it. */
export function wakeNote(world: string, n: MemoryNote): Promise<Outcome> {
  return act(`${base(world)}/memory/${encodeURIComponent(n.id)}/wake`, { seen: n.seen });
}

/** A settle's proof: a passing gate verdict by its number, or the human's own words. */
export type SettleProof = { verdict: number } | { statement: string };

/** Fact `f` settled on `proof`, as the human read it. */
export function settleFact(world: string, f: BoardFact, proof: SettleProof): Promise<Outcome> {
  return act(`${base(world)}/facts/${encodeURIComponent(f.id)}/settle`, { seen: f.seen, ...proof });
}
