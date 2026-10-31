/**
 * The command palette's data: every place it can jump to — the board's pages, each world and its
 * sections, the chats and past runs of the world the page is in — and the fuzzy match that ranks
 * them. A command only ever moves the page to an address; none answers, approves or acts.
 *
 * The match is one pass over the candidate for each query, linear in its length, and never builds
 * a regular expression from what was typed.
 */
import type { BoardRun, BoardSession, BoardWorld } from '@mnemorph/protocol';
import { BODIES_TABS, format, type Route, WORLD_SECTIONS, type WorldSection } from '../router.ts';

/** What a command is, for its group and its glyph. */
export type CommandKind = 'page' | 'world' | 'section' | 'session' | 'run';

/** A place the palette can jump to. */
export interface Command {
  /** Unique, stable across renders. */
  id: string;
  kind: CommandKind;
  /** The words matched and shown: a world's or a model's text, rendered inert. */
  label: string;
  /** Where it is, beside the label: the world's name. */
  detail: string;
  /** The address it moves to: always a board hash, never a script. */
  href: string;
}

/** The longest query the palette matches; past it the rest is ignored. */
export const QUERY_CAP = 120;
/** The most commands the palette lists. */
export const SHOWN_CAP = 50;

const SECTION_NAMES: Record<WorldSection, string> = {
  bodies: 'Bodies & queue',
  memory: 'Memory',
  ontology: 'Ontology',
  journal: 'Journal',
};

const TAB_NAMES: Record<(typeof BODIES_TABS)[number], string> = {
  colony: 'Colony net',
  races: 'Race cards',
  queue: 'Queue',
  timeline: 'Timeline',
};

const PAGES: [Route, string][] = [
  [{ name: 'worlds' }, 'Worlds'],
  [{ name: 'metrics' }, 'Metrics'],
  [{ name: 'integrations' }, 'Integrations'],
  [{ name: 'settings' }, 'Settings'],
];

/** Every command, in the palette's order when nothing is typed: pages, worlds, then chats. */
export function commands(
  worlds: readonly BoardWorld[],
  here: string | undefined,
  live: readonly BoardSession[],
  runs: readonly BoardRun[],
): Command[] {
  const out: Command[] = PAGES.map(([r, label]) => ({
    id: `page:${r.name}`,
    kind: 'page',
    label,
    detail: 'page',
    href: format(r),
  }));
  const name = (id: string) => worlds.find((w) => w.id === id)?.name ?? id;
  if (here !== undefined) {
    const liveRuns = new Set(live.map((s) => s.run));
    for (const s of live) {
      const r = runs.find((x) => x.id === s.run);
      out.push({
        id: `session:${s.id}`,
        kind: 'session',
        label: r?.ask || 'new chat',
        detail: `${name(here)} · live chat`,
        href: format({ name: 'session', world: here, session: s.id }),
      });
    }
    for (const r of runs) {
      if (liveRuns.has(r.id)) continue;
      out.push({
        id: `run:${r.id}`,
        kind: 'run',
        label: r.ask || r.id,
        detail: `${name(here)} · past run`,
        href: format({ name: 'run', world: here, run: r.id }),
      });
    }
  }
  for (const w of worlds) {
    out.push({
      id: `world:${w.id}`,
      kind: 'world',
      label: w.name,
      detail: w.state === 'missing' ? 'world · missing' : 'world',
      href: format({ name: 'world', world: w.id }),
    });
    for (const section of WORLD_SECTIONS) {
      out.push({
        id: `section:${w.id}:${section}`,
        kind: 'section',
        label: SECTION_NAMES[section],
        detail: w.name,
        href: format({ name: 'section', world: w.id, section }),
      });
    }
    for (const tab of BODIES_TABS) {
      if (tab === 'colony') continue; // the bodies section opens on it
      out.push({
        id: `section:${w.id}:bodies:${tab}`,
        kind: 'section',
        label: TAB_NAMES[tab],
        detail: w.name,
        href: format({ name: 'section', world: w.id, section: 'bodies', tab }),
      });
    }
  }
  return out;
}

/** A match: its score (higher is better) and the matched characters' places in the label. */
export interface Match {
  score: number;
  at: number[];
}

const isWordStart = (prev: string | undefined) =>
  prev === undefined ||
  prev === ' ' ||
  prev === '-' ||
  prev === '_' ||
  prev === '/' ||
  prev === '·';

/**
 * Whether every character of `query` appears in `text` in order, case-insensitively, and how
 * well: consecutive runs and word starts score up, gaps and a late start score down. One pass
 * over `text`; `at` counts code points of `text`.
 */
export function fuzzy(query: string, text: string): Match | undefined {
  const q = Array.from(query.slice(0, QUERY_CAP).trim().toLowerCase()).filter((c) => c !== ' ');
  if (q.length === 0) return { score: 0, at: [] };
  const t = Array.from(text);
  const at: number[] = [];
  let score = 0;
  let qi = 0;
  let last = -2;
  for (let i = 0; i < t.length && qi < q.length; i++) {
    const c = t[i] ?? '';
    if (c.toLowerCase() !== q[qi]) continue;
    score += 1;
    if (last === i - 1) score += 4;
    if (isWordStart(t[i - 1])) score += 3;
    if (at.length === 0) score -= Math.min(i, 10) * 0.25;
    else score -= Math.min(i - last - 1, 10) * 0.1;
    at.push(i);
    last = i;
    qi++;
  }
  return qi === q.length ? { score, at } : undefined;
}

/** A ranked command, with what of its label matched. */
export interface Ranked {
  command: Command;
  at: number[];
}

/**
 * The commands `query` matches, best first, at most [`SHOWN_CAP`]. The label is matched first;
 * the detail (a world's name) counts at a discount, so "memory sql" finds sqlglot's memory.
 */
export function rank(all: readonly Command[], query: string): Ranked[] {
  const q = query.slice(0, QUERY_CAP).trim();
  if (q === '') return all.slice(0, SHOWN_CAP).map((command) => ({ command, at: [] }));
  const scored: { r: Ranked; score: number; i: number }[] = [];
  all.forEach((command, i) => {
    const onLabel = fuzzy(q, command.label);
    if (onLabel) {
      scored.push({ r: { command, at: onLabel.at }, score: onLabel.score + 2, i });
      return;
    }
    const both = fuzzy(q, `${command.label} ${command.detail}`);
    if (both) {
      const inLabel = Array.from(command.label).length;
      scored.push({
        r: { command, at: both.at.filter((x) => x < inLabel) },
        score: both.score,
        i,
      });
    }
  });
  scored.sort((a, b) => b.score - a.score || a.i - b.i);
  return scored.slice(0, SHOWN_CAP).map((s) => s.r);
}

/** `text` cut at the matched places: runs of plain and matched code points, in order. */
export function marked(text: string, at: readonly number[]): { text: string; hit: boolean }[] {
  const t = Array.from(text);
  const hits = new Set(at);
  const out: { text: string; hit: boolean }[] = [];
  for (let i = 0; i < t.length; i++) {
    const hit = hits.has(i);
    const prev = out[out.length - 1];
    if (prev && prev.hit === hit) prev.text += t[i];
    else out.push({ text: t[i] ?? '', hit });
  }
  return out;
}
