/**
 * A world's colony, queue, timeline and journal (`GET /api/w/<id>/bodies`, `/queue`, `/timeline`,
 * `/journal`), generated from Rust, read again whenever the push channel names the world. Each
 * answer is checked before the page uses it: a row that does not hold together is left out and
 * counted, never guessed at. Everything but the race law and the witness's word is read inside the
 * world: shown, granting nothing.
 */
import type {
  BoardBodies,
  BoardBody,
  BoardJournal,
  BoardOrder,
  BoardQueue,
  BoardTimeline,
  BodyMind,
  BodyState,
  CommissionState,
  JournalEntry,
  JournalToc,
  MindStanding,
  OrderState,
  Provenance,
  RaceLaw,
  TimelineEntry,
  TimelineKind,
} from '@mnemorph/protocol';
import { useKnowledge } from './knowledge.ts';

const obj = (v: unknown): Record<string, unknown> | undefined =>
  typeof v === 'object' && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : undefined;
const count = (v: unknown): v is number => typeof v === 'number' && Number.isInteger(v) && v >= 0;
const int = (v: unknown): v is number => typeof v === 'number' && Number.isInteger(v);
const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const str = (v: unknown): v is string => typeof v === 'string';
const strOrNull = (v: unknown): v is string | null => v === null || str(v);
const bool = (v: unknown): v is boolean => typeof v === 'boolean';
const strs = (v: unknown): v is string[] => Array.isArray(v) && v.every(str);
const oneOf =
  <T extends string>(all: readonly T[]) =>
  (v: unknown): v is T =>
    all.includes(v as T);

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

export const BODY_STATES: readonly BodyState[] = ['working', 'idle', 'cut', 'never', 'retired'];
export const MIND_STANDINGS: readonly MindStanding[] = [
  'adopted',
  'unadopted',
  'changed',
  'retired',
  'unverified',
  'absent',
];
export const ORDER_STATES: readonly OrderState[] = ['held', 'claimed', 'done', 'dropped'];
export const TIMELINE_KINDS: readonly TimelineKind[] = ['commission', 'verdict'];
const COMMISSION_STATES: readonly CommissionState[] = ['ended', 'open', 'unknown'];

function provenance(v: unknown): Provenance | undefined {
  const o = obj(v);
  return o && str(o.source) && bool(o.trusted) && strOrNull(o.why)
    ? { source: o.source, trusted: o.trusted, why: o.why }
    : undefined;
}

/** One entry of the timeline, or `undefined`. */
export function entry(v: unknown): TimelineEntry | undefined {
  const o = obj(v);
  if (!o || !oneOf(TIMELINE_KINDS)(o.kind) || !str(o.at) || !str(o.body)) return undefined;
  if (!strOrNull(o.run) || !(o.n === null || count(o.n)) || !strOrNull(o.ask)) return undefined;
  if (!(o.state === null || oneOf(COMMISSION_STATES)(o.state))) return undefined;
  if (!strOrNull(o.ending) || !strOrNull(o.verdict) || !count(o.changed)) return undefined;
  if (!strs(o.failures) || !strOrNull(o.why) || !str(o.source) || !bool(o.trusted)) {
    return undefined;
  }
  return {
    kind: o.kind,
    at: o.at,
    body: o.body,
    run: o.run,
    n: o.n,
    ask: o.ask,
    state: o.state,
    ending: o.ending,
    verdict: o.verdict,
    changed: o.changed,
    failures: o.failures,
    why: o.why,
    source: o.source,
    trusted: o.trusted,
  };
}

function mind(v: unknown): BodyMind | undefined {
  const o = obj(v);
  return o &&
    str(o.name) &&
    oneOf(MIND_STANDINGS)(o.standing) &&
    bool(o.shared) &&
    str(o.about) &&
    strOrNull(o.owner)
    ? { name: o.name, standing: o.standing, shared: o.shared, about: o.about, owner: o.owner }
    : undefined;
}

function body(v: unknown): BoardBody | undefined {
  const o = obj(v);
  if (!o || !str(o.name) || !str(o.race) || !str(o.lane) || !strOrNull(o.parent)) return undefined;
  if (!strOrNull(o.post) || !strOrNull(o.speciality) || !count(o.verify) || !bool(o.retired)) {
    return undefined;
  }
  if (!strOrNull(o.heir_of) || !str(o.doc)) return undefined;
  const live = obj(o.live);
  if (!live || !oneOf(BODY_STATES)(live.state) || !str(live.why)) return undefined;
  if (!strOrNull(live.run) || !strOrNull(live.at)) return undefined;
  const ground = rows(o.ground, (r) => {
    const x = obj(r);
    return x && str(x.kind) && str(x.path) ? { kind: x.kind, path: x.path } : undefined;
  });
  const minds = rows(o.minds, mind);
  const acts = rows(o.acts, entry);
  if (ground.bad + minds.bad + acts.bad > 0) return undefined;
  return {
    name: o.name,
    race: o.race,
    lane: o.lane,
    parent: o.parent,
    post: o.post,
    speciality: o.speciality,
    ground: ground.ok,
    verify: o.verify,
    retired: o.retired,
    heir_of: o.heir_of,
    doc: o.doc,
    minds: minds.ok,
    live: { state: live.state, why: live.why, run: live.run, at: live.at },
    acts: acts.ok,
  };
}

function race(v: unknown): RaceLaw | undefined {
  const o = obj(v);
  if (!o || !str(o.race) || !str(o.title) || !str(o.base) || !bool(o.ascended)) return undefined;
  if (!strOrNull(o.ascends_to) || !strOrNull(o.reports_to) || !strs(o.commands)) return undefined;
  if (!bool(o.sideways) || !strs(o.shelf) || !strs(o.soil) || !str(o.gate)) return undefined;
  if (!bool(o.writes_code) || !count(o.steps) || !count(o.max_share) || !str(o.tier)) {
    return undefined;
  }
  if (!count(o.living) || !str(o.art)) return undefined;
  const m = obj(o.meter);
  if (!m || !str(m.label) || !num(m.ok) || !num(m.warn)) return undefined;
  const bars = rows(o.bars, (r) => {
    const x = obj(r);
    return x && str(x.kind) && str(x.meter) && str(x.scale) && str(x.op) && num(x.threshold)
      ? { kind: x.kind, meter: x.meter, scale: x.scale, op: x.op, threshold: x.threshold }
      : undefined;
  });
  const posts = rows(o.posts, (r) => {
    const x = obj(r);
    return x && str(x.creature) && strs(x.acts) ? { creature: x.creature, acts: x.acts } : undefined;
  });
  const specialities = rows(o.specialities, (r) => {
    const x = obj(r);
    return x && str(x.creature) && strs(x.classes) && strs(x.surfaces)
      ? { creature: x.creature, classes: x.classes, surfaces: x.surfaces }
      : undefined;
  });
  if (bars.bad + posts.bad + specialities.bad > 0) return undefined;
  return {
    race: o.race,
    title: o.title,
    base: o.base,
    ascended: o.ascended,
    ascends_to: o.ascends_to,
    reports_to: o.reports_to,
    commands: o.commands,
    sideways: o.sideways,
    shelf: o.shelf,
    soil: o.soil,
    gate: o.gate,
    writes_code: o.writes_code,
    steps: o.steps,
    max_share: o.max_share,
    tier: o.tier,
    meter: { label: m.label, ok: m.ok, warn: m.warn },
    bars: bars.ok,
    posts: posts.ok,
    specialities: specialities.ok,
    living: o.living,
    art: o.art,
  };
}

export interface BodiesPage extends BoardBodies {
  /** Rows the answer held that did not read. */
  unreadable: number;
}

/** The colony an answer holds; `undefined` when it is not one at all. */
export function parseBodies(v: unknown): BodiesPage | undefined {
  const o = obj(v);
  if (!o || !strs(o.problems) || !strs(o.loops)) return undefined;
  const throne = obj(o.throne);
  if (!throne || !bool(throne.live) || !str(throne.why)) return undefined;
  const p = ['roster', 'minds', 'live', 'acts', 'law'].map((k) => provenance(o[k]));
  const [roster, minds, live, acts, law] = p;
  if (!roster || !minds || !live || !acts || !law) return undefined;
  const bodies = rows(o.bodies, body);
  const races = rows(o.races, race);
  return {
    bodies: bodies.ok,
    problems: o.problems,
    loops: o.loops,
    throne: { live: throne.live, why: throne.why },
    races: races.ok,
    roster,
    minds,
    live,
    acts,
    law,
    unreadable: bodies.bad + races.bad,
  };
}

export interface QueuePage extends BoardQueue {
  unreadable: number;
}

function order(v: unknown): BoardOrder | undefined {
  const o = obj(v);
  if (!o || !str(o.id) || !count(o.rev) || !oneOf(ORDER_STATES)(o.state)) return undefined;
  if (!str(o.holder) || !bool(o.throne) || !str(o.by) || !int(o.rank) || !str(o.ask)) {
    return undefined;
  }
  if (!strs(o.notes) || !(o.witnessed === null || bool(o.witnessed))) return undefined;
  return {
    id: o.id,
    rev: o.rev,
    state: o.state,
    holder: o.holder,
    throne: o.throne,
    by: o.by,
    rank: o.rank,
    ask: o.ask,
    notes: o.notes,
    witnessed: o.witnessed,
  };
}

/** The queue an answer holds; `undefined` when it is not one at all. */
export function parseQueue(v: unknown): QueuePage | undefined {
  const o = obj(v);
  const c = obj(o?.counts);
  if (!o || !c || !str(o.source) || !strOrNull(o.witness) || !strOrNull(o.witness_why)) {
    return undefined;
  }
  if (!count(c.held) || !count(c.claimed) || !count(c.done) || !count(c.dropped)) {
    return undefined;
  }
  const orders = rows(o.orders, order);
  return {
    orders: orders.ok,
    counts: { held: c.held, claimed: c.claimed, done: c.done, dropped: c.dropped },
    source: o.source,
    witness: o.witness,
    witness_why: o.witness_why,
    unreadable: orders.bad,
  };
}

export interface TimelinePage extends BoardTimeline {
  unreadable: number;
}

/** The timeline an answer holds; `undefined` when it is not one at all. */
export function parseTimeline(v: unknown): TimelinePage | undefined {
  const o = obj(v);
  if (!o || !count(o.total) || !count(o.offset) || !count(o.limit) || !strs(o.bodies)) {
    return undefined;
  }
  if (!count(o.commissions) || !count(o.verdicts) || !count(o.older)) return undefined;
  if (!strOrNull(o.verdicts_why)) return undefined;
  const entries = rows(o.entries, entry);
  return {
    entries: entries.ok,
    total: o.total,
    offset: o.offset,
    limit: o.limit,
    bodies: o.bodies,
    commissions: o.commissions,
    verdicts: o.verdicts,
    older: o.older,
    verdicts_why: o.verdicts_why,
    unreadable: entries.bad,
  };
}

export interface JournalPage extends BoardJournal {
  unreadable: number;
}

function journalEntry(v: unknown): JournalEntry | undefined {
  const o = obj(v);
  return o &&
    count(o.n) &&
    str(o.heading) &&
    strOrNull(o.at) &&
    strOrNull(o.body) &&
    str(o.title) &&
    str(o.text) &&
    bool(o.cut)
    ? {
        n: o.n,
        heading: o.heading,
        at: o.at,
        body: o.body,
        title: o.title,
        text: o.text,
        cut: o.cut,
      }
    : undefined;
}

function toc(v: unknown): JournalToc | undefined {
  const o = obj(v);
  return o && count(o.n) && strOrNull(o.at) && strOrNull(o.body) && str(o.title)
    ? { n: o.n, at: o.at, body: o.body, title: o.title }
    : undefined;
}

/** The journal an answer holds; `undefined` when it is not one at all. */
export function parseJournal(v: unknown): JournalPage | undefined {
  const o = obj(v);
  if (!o || !count(o.total) || !count(o.all) || !count(o.offset) || !count(o.limit)) {
    return undefined;
  }
  if (!strOrNull(o.q) || !bool(o.older) || !(o.bytes === null || count(o.bytes))) {
    return undefined;
  }
  if (!str(o.source)) return undefined;
  const entries = rows(o.entries, journalEntry);
  const lines = rows(o.toc, toc);
  return {
    entries: entries.ok,
    toc: lines.ok,
    total: o.total,
    all: o.all,
    offset: o.offset,
    limit: o.limit,
    q: o.q,
    older: o.older,
    bytes: o.bytes,
    source: o.source,
    unreadable: entries.bad + lines.bad,
  };
}

const base = (id: string) => `/api/w/${encodeURIComponent(id)}`;

export function useBodies(id: string, again = 0) {
  return useKnowledge(id, `${base(id)}/bodies`, parseBodies, again);
}

export function useQueue(id: string, again = 0) {
  return useKnowledge(id, `${base(id)}/queue`, parseQueue, again);
}

/** What the timeline page asks for. */
export interface TimelineQuery {
  kind?: TimelineKind | undefined;
  body?: string | undefined;
  offset?: number | undefined;
}

export function timelineQuery(q: TimelineQuery): string {
  const p = new URLSearchParams();
  if (q.kind) p.set('kind', q.kind);
  if (q.body) p.set('body', q.body);
  if (q.offset) p.set('offset', String(q.offset));
  const s = p.toString();
  return s === '' ? '' : `?${s}`;
}

export function useTimeline(id: string, q: TimelineQuery, again = 0) {
  return useKnowledge(id, `${base(id)}/timeline${timelineQuery(q)}`, parseTimeline, again);
}

/** What the journal page asks for. */
export interface JournalQuery {
  q?: string | undefined;
  offset?: number | undefined;
}

export function journalQuery(q: JournalQuery): string {
  const p = new URLSearchParams();
  if (q.q) p.set('q', q.q);
  if (q.offset) p.set('offset', String(q.offset));
  const s = p.toString();
  return s === '' ? '' : `?${s}`;
}

export function useJournal(id: string, q: JournalQuery, again = 0) {
  return useKnowledge(id, `${base(id)}/journal${journalQuery(q)}`, parseJournal, again);
}
