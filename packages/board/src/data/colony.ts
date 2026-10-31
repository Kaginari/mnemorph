/**
 * The colony net's model, apart from its drawing: race lanes in the law's order, the chain of
 * command as edges, a position for every node, and a label for every node. Pure data — the SVG
 * (`views/ColonyNet.tsx`) only draws what this returns.
 *
 * The edges are the roster's `Reports to`, as the docs say them; a doc names nobody, or a body
 * whose race reports to the throne, hangs from the throne. Nothing in a doc can make the drawing
 * loop: an edge that would close a cycle is left out and said, as is one to a name no body has.
 */
import type { BoardBody, BodyState, RaceLaw, ThroneLive } from '@mnemorph/protocol';

/** The lanes, top to bottom: the throne, then the races as the law lists them (base races). */
export const LANES = ['rimuru', 'elf', 'orc', 'slime', 'kijin', 'dark-elf'] as const;

/** A node's box, and the grid it sits on. */
export const NODE_W = 204;
export const NODE_H = 46;
export const COL = 224;
export const ROW = 92;
export const GUTTER = 112;

/** The words of each live state, beside its glyph: never colour alone. */
export const STATE_WORDS: Record<BodyState, { glyph: string; words: string }> = {
  working: { glyph: '●', words: 'working' },
  idle: { glyph: '○', words: 'idle' },
  cut: { glyph: '◐', words: 'cut mid-turn' },
  never: { glyph: '·', words: 'not commissioned yet' },
  retired: { glyph: '☾', words: 'retired' },
};

export interface ColonyNode {
  /** The body's name; the throne's own when none is in the roster. */
  name: string;
  race: string;
  lane: string;
  /** The race's title (`High Orc`). */
  title: string;
  state: BodyState;
  /** Every word a reader needs, without colour: name, race, post or speciality, state. */
  label: string;
  /** No doc describes it: the throne drawn for the session. */
  synthetic: boolean;
  x: number;
  y: number;
}

export interface ColonyEdge {
  /** The commander. */
  from: string;
  /** The commanded. */
  to: string;
}

export interface Dropped {
  from: string;
  to: string;
  why: 'loop' | 'unknown';
}

export interface ColonyLane {
  race: string;
  title: string;
  y: number;
  nodes: string[];
}

export interface Colony {
  lanes: ColonyLane[];
  nodes: ColonyNode[];
  edges: ColonyEdge[];
  dropped: Dropped[];
  width: number;
  height: number;
}

/** The throne's name when no doc describes it. */
export const THRONE = 'rimuru';

function laneRank(lane: string): number {
  const i = (LANES as readonly string[]).indexOf(lane);
  return i < 0 ? LANES.length : i;
}

/** The lanes' order: the law's, then any other alphabetically. */
export function sortLanes(lanes: Iterable<string>): string[] {
  return [...new Set(lanes)].sort((a, b) => laneRank(a) - laneRank(b) || a.localeCompare(b));
}

/** Whether `ancestor` is up the chain from `name` in `parent`. */
function above(parent: Map<string, string>, ancestor: string, name: string): boolean {
  const seen = new Set<string>();
  for (let at: string | undefined = name; at !== undefined; at = parent.get(at)) {
    if (at === ancestor) return true;
    if (seen.has(at)) return false;
    seen.add(at);
  }
  return false;
}

/** The colony of `bodies` under the law of `races`, the throne `throne`. */
export function colony(bodies: BoardBody[], races: RaceLaw[], throne: ThroneLive): Colony {
  const law = new Map(races.map((r) => [r.race, r]));
  const titleOf = (race: string) => law.get(race)?.title ?? race;
  const sorted = [...bodies].sort((a, b) => a.name.localeCompare(b.name));
  const names = new Set(sorted.map((b) => b.name));
  const thrones = sorted.filter((b) => b.race === 'rimuru' && !b.retired);
  const throneName = thrones[0]?.name ?? THRONE;
  const synthetic = thrones.length === 0 && !names.has(THRONE);

  const raw: Omit<ColonyNode, 'x' | 'y'>[] = sorted.map((b) => {
    const role = b.post ? `post ${b.post}` : b.speciality ? `speciality ${b.speciality}` : '';
    const state = STATE_WORDS[b.live.state].words;
    return {
      name: b.name,
      race: b.race,
      lane: b.lane,
      title: titleOf(b.race),
      state: b.live.state,
      label: [b.name, titleOf(b.race), role, state].filter((w) => w !== '').join(' · '),
      synthetic: false,
    };
  });
  if (synthetic) {
    const state: BodyState = throne.live ? 'working' : 'idle';
    raw.unshift({
      name: THRONE,
      race: 'rimuru',
      lane: 'rimuru',
      title: titleOf('rimuru'),
      state,
      label: `${THRONE} · the throne · ${STATE_WORDS[state].words}`,
      synthetic: true,
    });
  }

  // The chain of command, each edge kept only if it closes no loop.
  const parent = new Map<string, string>();
  const edges: ColonyEdge[] = [];
  const dropped: Dropped[] = [];
  for (const b of sorted) {
    let up = b.parent ?? undefined;
    if (up === undefined) {
      const reports = law.get(b.race)?.reports_to;
      if (reports === 'rimuru' && b.race !== 'rimuru' && b.name !== throneName) up = throneName;
    }
    if (up === undefined) continue;
    if (up !== throneName && !names.has(up)) {
      dropped.push({ from: up, to: b.name, why: 'unknown' });
      continue;
    }
    if (up === b.name || above(parent, b.name, up)) {
      dropped.push({ from: up, to: b.name, why: 'loop' });
      continue;
    }
    parent.set(b.name, up);
    edges.push({ from: up, to: b.name });
  }

  // Lane by lane, top to bottom; within one, by the commander's place, then by name.
  const lanes: ColonyLane[] = [];
  const x = new Map<string, number>();
  const out: ColonyNode[] = [];
  sortLanes(raw.map((n) => n.lane)).forEach((lane, li) => {
    const here = raw
      .filter((n) => n.lane === lane)
      .sort((a, b) => {
        const pa = x.get(parent.get(a.name) ?? '') ?? Number.POSITIVE_INFINITY;
        const pb = x.get(parent.get(b.name) ?? '') ?? Number.POSITIVE_INFINITY;
        if (pa !== pb) return pa < pb ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
    const y = li * ROW + ROW / 2;
    here.forEach((n, i) => {
      const nx = GUTTER + i * COL + NODE_W / 2;
      x.set(n.name, nx);
      out.push({ ...n, x: nx, y });
    });
    lanes.push({ race: lane, title: titleOf(lane), y, nodes: here.map((n) => n.name) });
  });
  const widest = Math.max(1, ...lanes.map((l) => l.nodes.length));
  return {
    lanes,
    nodes: out,
    edges,
    dropped,
    width: GUTTER + widest * COL,
    height: Math.max(1, lanes.length) * ROW,
  };
}
