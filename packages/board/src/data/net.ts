/**
 * The colony net page's model (`#/net`): the colony as a hex city around the throne. The six races
 * and their five ascended forms stand on a terraced lattice, the same in every world, and a world's
 * own bodies, live, are seated around their race.
 *
 * Pure layout and 3D geometry only; the canvas (`views/Net.tsx`) keeps no logic a test could
 * reach. jsdom has no canvas, so everything here runs under plain vitest.
 *
 * The colony's shape (docs/02-races.md): Veldora (the human) above the throne, Rimuru on the
 * city's peak, the chain Elf → Orc → Slime running down one terrace, Kijin and Dark Elf out on the
 * flanks, and each base's ascended form beside it, joined by a dashed ascension edge.
 */

export type NodeKind = 'human' | 'throne' | 'base' | 'ascended' | 'body';

export interface ColonyNode {
  id: string;
  name: string;
  /** The race's job, in one line (docs/02-races.md's plain-words table). */
  job: string;
  /** The race's colour token (`--mn-race-*` in @mnemorph/theme's tokens.css). */
  token: string;
  /** World position, in the layout's own units. */
  position: readonly [number, number, number];
  kind: NodeKind;
  /** For an ascended form, the base it rose from. */
  ascendsFrom?: string;
}

export interface ColonyEdge {
  from: string;
  to: string;
  /** command: who commissions; ascends: who rose from whom; seat: a race and a body of it. */
  kind: 'command' | 'ascends' | 'seat';
}

export type Vec = readonly [number, number, number];

/** The city's radius, in the camera's units. */
export const CITY = 3.2;

/** The terrain's height at a point of the unit disc: a terraced hill, the throne on its peak. */
export function terrace(x: number, z: number): number {
  const ring = Math.min(1, Math.hypot(x, z));
  return 0.42 * (1 - ring) ** 1.6 - 0.2;
}

/** A point standing `rise` above the terrain, at a place of the unit disc, in the camera's units. */
const on = (x: number, z: number, rise = 0.05): Vec => [
  x * CITY,
  (terrace(x, z) + rise) * CITY,
  z * CITY,
];

export const NODES: readonly ColonyNode[] = [
  {
    id: 'veldora',
    name: 'Veldora',
    job: 'the human — everything answers to you',
    token: '--mn-race-veldora',
    position: [0, 0.84 * CITY, 0],
    kind: 'human',
  },
  {
    id: 'rimuru',
    name: 'Rimuru',
    job: 'understand the human, plan, answer',
    token: '--mn-race-rimuru',
    position: on(0, 0, 0.16),
    kind: 'throne',
  },
  {
    id: 'elf',
    name: 'Elf',
    job: 'plan one lane of work across domains',
    token: '--mn-race-elf',
    position: on(0, 0.34),
    kind: 'base',
  },
  {
    id: 'orc',
    name: 'Orc',
    job: 'own a domain — the test first, judge the landing',
    token: '--mn-race-orc',
    position: on(0, 0.6),
    kind: 'base',
  },
  {
    id: 'slime',
    name: 'Slime',
    job: 'change the code of its one zone, with its doc',
    token: '--mn-race-slime',
    position: on(0, 0.86),
    kind: 'base',
  },
  {
    id: 'kijin',
    name: 'Kijin',
    job: 'everything that leaves the machine — CI, releases, deploys',
    token: '--mn-race-kijin',
    position: on(-0.52, -0.3),
    kind: 'base',
  },
  {
    id: 'darkElf',
    name: 'Dark Elf',
    job: 'attack the code; prove every weakness it claims',
    token: '--mn-race-dark-elf',
    position: on(0.52, -0.3),
    kind: 'base',
  },
  {
    id: 'highElf',
    name: 'High Elf',
    job: 'the Elf ascended — the same lane, less friction',
    token: '--mn-race-high-elf',
    position: on(0.42, 0.4),
    kind: 'ascended',
    ascendsFrom: 'elf',
  },
  {
    id: 'highOrc',
    name: 'High Orc',
    job: 'the Orc ascended — the same domain, less friction',
    token: '--mn-race-high-orc',
    position: on(0.52, 0.62),
    kind: 'ascended',
    ascendsFrom: 'orc',
  },
  {
    id: 'namedSlime',
    name: 'Named Slime',
    job: 'the Slime ascended — the same zone, less friction',
    token: '--mn-race-named-slime',
    position: on(0.6, 0.78),
    kind: 'ascended',
    ascendsFrom: 'slime',
  },
  {
    id: 'oni',
    name: 'Oni',
    job: 'the Kijin ascended — the same reach, less friction',
    token: '--mn-race-oni',
    position: on(-0.8, -0.46),
    kind: 'ascended',
    ascendsFrom: 'kijin',
  },
  {
    id: 'archDarkElf',
    name: 'Arch Dark Elf',
    job: 'the Dark Elf ascended — the same ground, less friction',
    token: '--mn-race-arch-dark-elf',
    position: on(0.86, -0.2),
    kind: 'ascended',
    ascendsFrom: 'darkElf',
  },
];

export const EDGES: readonly ColonyEdge[] = [
  { from: 'veldora', to: 'rimuru', kind: 'command' },
  { from: 'rimuru', to: 'elf', kind: 'command' },
  { from: 'rimuru', to: 'kijin', kind: 'command' },
  { from: 'rimuru', to: 'darkElf', kind: 'command' },
  { from: 'elf', to: 'orc', kind: 'command' },
  { from: 'orc', to: 'slime', kind: 'command' },
  { from: 'elf', to: 'highElf', kind: 'ascends' },
  { from: 'orc', to: 'highOrc', kind: 'ascends' },
  { from: 'slime', to: 'namedSlime', kind: 'ascends' },
  { from: 'kijin', to: 'oni', kind: 'ascends' },
  { from: 'darkElf', to: 'archDarkElf', kind: 'ascends' },
];

/** A node's colour, for CSS: its race's token. */
export function colour(n: ColonyNode): string {
  return `var(${n.token})`;
}

export function node(id: string): ColonyNode | undefined {
  return NODES.find((n) => n.id === id);
}

/** The chain a node sits on: one of the six races (an ascended form's base). */
export function baseOf(n: ColonyNode): ColonyNode {
  return n.ascendsFrom ? (node(n.ascendsFrom) ?? n) : n;
}

/* ── 3D ─────────────────────────────────────────────────────────── */

export interface Camera {
  /** Rotation around the world's Y axis, radians. */
  yaw: number;
  /** Rotation around the world's X axis, radians. */
  pitch: number;
  /** Screen scale factor. */
  zoom: number;
}

export const DEFAULT_CAMERA: Camera = { yaw: 0.4, pitch: 0.58, zoom: 1 };
export const MIN_ZOOM = 0.45;
export const MAX_ZOOM = 3;
export const MIN_PITCH = -1.25;
export const MAX_PITCH = 1.25;
export const FOV = 9;
export const NODE_RADIUS = 0.24;
/** A body's hex, beside a race's. */
export const BODY_RADIUS = 0.13;

function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}

/** A new camera yawed and pitched by the deltas, pitches clamped. */
export function orbit(camera: Camera, dYaw: number, dPitch: number): Camera {
  return {
    ...camera,
    yaw: camera.yaw + dYaw,
    pitch: clamp(camera.pitch + dPitch, MIN_PITCH, MAX_PITCH),
  };
}

/** A new camera with zoom scaled by `factor`, clamped. */
export function zoomed(camera: Camera, factor: number): Camera {
  return { ...camera, zoom: clamp(camera.zoom * factor, MIN_ZOOM, MAX_ZOOM) };
}

/** Auto-rotation: advance yaw by dt seconds; `allow` is false under prefers-reduced-motion. */
export function spin(camera: Camera, dtSeconds: number, allow: boolean, speed = 0.25): Camera {
  if (!allow) return camera;
  return { ...camera, yaw: camera.yaw + dtSeconds * speed };
}

/** A point after the camera's yaw-then-pitch rotation; `z` is the depth (larger = nearer). */
export interface Rotated {
  x: number;
  y: number;
  z: number;
}

export function rotate(point: readonly [number, number, number], camera: Camera): Rotated {
  const [x0, y0, z0] = point;
  const cy = Math.cos(camera.yaw);
  const sy = Math.sin(camera.yaw);
  const x = x0 * cy + z0 * sy;
  const z1 = -x0 * sy + z0 * cy;
  const cp = Math.cos(camera.pitch);
  const sp = Math.sin(camera.pitch);
  return { x, y: y0 * cp - z1 * sp, z: y0 * sp + z1 * cp };
}

/**
 * Perspective scale at a depth (larger = nearer): FOV/(FOV−z), 1 at z=0; between ~0.7 and ~1.9 for
 * this layout's nodes, from any camera. Only defined in front of the eye (z < FOV): the canvas clips nearer.
 */
export function scaleAt(z: number): number {
  return FOV / (FOV - z);
}

/** Pixels to one unit of the world: the city fills the stage, whichever side is shorter. */
function unitOf(width: number, height: number): number {
  return Math.min(height / 7.4, width / 7.6);
}

/** A point rotated and projected to screen pixels. */
export interface Screen extends Rotated {
  scale: number;
  sx: number;
  sy: number;
}

export function toScreen(
  point: readonly [number, number, number],
  camera: Camera,
  width: number,
  height: number,
): Screen {
  const r = rotate(point, camera);
  const scale = scaleAt(r.z);
  const unit = unitOf(width, height) * camera.zoom;
  return { ...r, scale, sx: width / 2 + r.x * scale * unit, sy: height / 2 - r.y * scale * unit };
}

/** A node's projected circle on screen. */
export interface Projected {
  node: ColonyNode;
  x: number;
  y: number;
  radius: number;
  depth: number;
}

export function project(
  node: ColonyNode,
  camera: Camera,
  width: number,
  height: number,
): Projected {
  const s = toScreen(node.position, camera, width, height);
  return {
    node,
    x: s.sx,
    y: s.sy,
    radius: Math.max(
      (node.kind === 'body' ? BODY_RADIUS : NODE_RADIUS) *
        s.scale *
        unitOf(width, height) *
        camera.zoom,
      1.5,
    ),
    depth: s.z,
  };
}

/** Paint order: far to near (depth ascending), the painter's algorithm. */
export function paintOrder(items: readonly Projected[]): Projected[] {
  return [...items].sort((a, b) => a.depth - b.depth);
}

/** The node nearest to a screen point within maxDist; nearest wins on ties. */
export function nearest(
  items: readonly Projected[],
  x: number,
  y: number,
  maxDist = 40,
): Projected | undefined {
  let best: Projected | undefined;
  let bestD = maxDist;
  for (const item of items) {
    const d = Math.hypot(item.x - x, item.y - y);
    if (d <= bestD) {
      best = item;
      bestD = d;
    }
  }
  return best;
}

/* ── the city ───────────────────────────────────────────────────────────────────────────── */

const HEX_RINGS = 8;

export interface Lattice {
  /** The corners of every cell, on the terrain. */
  points: readonly Vec[];
  /** The cells' edges, as pairs of points. */
  links: readonly (readonly [number, number])[];
}

/** The hex city: rings of cells around the throne, each corner shared with its neighbours. */
export const LATTICE: Lattice = (() => {
  const cell = 1 / (HEX_RINGS * 1.75);
  const points: Vec[] = [];
  const seats = new Map<string, number>();
  const links = new Set<string>();
  const seat = (x: number, z: number) => {
    const key = `${x.toFixed(3)},${z.toFixed(3)}`;
    const held = seats.get(key);
    if (held !== undefined) return held;
    points.push([x * CITY, terrace(x, z) * CITY, z * CITY]);
    seats.set(key, points.length - 1);
    return points.length - 1;
  };
  for (let q = -HEX_RINGS; q <= HEX_RINGS; q++) {
    const from = Math.max(-HEX_RINGS, -q - HEX_RINGS);
    const to = Math.min(HEX_RINGS, -q + HEX_RINGS);
    for (let r = from; r <= to; r++) {
      const cx = cell * Math.sqrt(3) * (q + r / 2);
      const cz = cell * 1.5 * r;
      const corners = Array.from({ length: 6 }, (_, c) => {
        const angle = (Math.PI / 3) * c + Math.PI / 6;
        return seat(cx + cell * Math.cos(angle), cz + cell * Math.sin(angle));
      });
      corners.forEach((a, i) => {
        const b = corners[(i + 1) % 6] ?? a;
        links.add(a < b ? `${a}-${b}` : `${b}-${a}`);
      });
    }
  }
  return {
    points,
    links: [...links].map((pair) => {
      const [a = 0, b = 0] = pair.split('-').map(Number);
      return [a, b] as const;
    }),
  };
})();

/** A ripple through the city, from where something happened. */
export interface Wave {
  origin: Vec;
  /** When it began, in milliseconds. */
  start: number;
  /** A race's token, or none for the interface's own colour. */
  token?: string | undefined;
}

const WAVE_SPEED = 1.5 * CITY;
const WAVE_WIDTH = 0.2 * CITY;
export const WAVE_LIFE_S = 2;

const gap = (a: Vec, b: Vec) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

/** How strongly the waves light a point at `now`, 0 to 1, and which one does. */
export function energy(
  point: Vec,
  waves: readonly Wave[],
  now: number,
): { power: number; wave?: Wave } {
  let lit: { power: number; wave?: Wave } = { power: 0 };
  for (const wave of waves) {
    const age = (now - wave.start) / 1000;
    if (age < 0 || age > WAVE_LIFE_S) continue;
    const off = Math.abs(gap(point, wave.origin) - age * WAVE_SPEED);
    if (off > WAVE_WIDTH) continue;
    const power = (1 - off / WAVE_WIDTH) * (1 - age / WAVE_LIFE_S);
    if (power > lit.power) lit = { power, wave };
  }
  return lit;
}

/** The race of a node as the law names it (`high-orc`), from its token. */
export function raceOf(n: ColonyNode): string {
  return n.token.replace('--mn-race-', '');
}

/** The body fields the city reads. */
export interface Seatable {
  name: string;
  race: string;
  retired: boolean;
  working: boolean;
}

/** The most bodies seated around one race: past it, the Bodies page holds the rest. */
const SEATS = 12;

/** The net of a world: the races, and each living body seated around its race. */
export function netOf(bodies: readonly Seatable[]): {
  nodes: readonly ColonyNode[];
  edges: readonly ColonyEdge[];
  working: ReadonlySet<string>;
} {
  const nodes: ColonyNode[] = [...NODES];
  const edges: ColonyEdge[] = [...EDGES];
  const working = new Set<string>();
  const seated = new Map<string, number>();
  for (const b of bodies) {
    const race = NODES.find((n) => raceOf(n) === b.race && n.kind !== 'throne');
    if (b.retired || !race || race.kind === 'human') continue;
    const seat = seated.get(race.id) ?? 0;
    if (seat >= SEATS) continue;
    seated.set(race.id, seat + 1);
    const angle = seat * 2.4 + 0.8;
    const reach = (0.11 + 0.012 * seat) * CITY;
    const [x, , z] = race.position;
    const px = x + reach * Math.cos(angle);
    const pz = z + reach * Math.sin(angle);
    const id = `body:${b.name}`;
    nodes.push({
      id,
      name: b.name,
      job: `a body of ${race.name}`,
      token: race.token,
      position: on(px / CITY, pz / CITY, 0.09),
      kind: 'body',
    });
    edges.push({ from: race.id, to: id, kind: 'seat' });
    if (b.working) working.add(id);
  }
  return { nodes, edges, working };
}

/** Who `id` reaches: the nodes one edge away. */
export function related(id: string | undefined, edges: readonly ColonyEdge[]): Set<string> {
  const out = new Set<string>();
  if (id === undefined) return out;
  for (const e of edges) {
    if (e.from === id) out.add(e.to);
    if (e.to === id) out.add(e.from);
  }
  return out;
}

/** How far a street of the city lies from a line of reach, on the ground. */
function offLine(point: Vec, from: Vec, to: Vec): number {
  const dx = to[0] - from[0];
  const dz = to[2] - from[2];
  const length = dx * dx + dz * dz;
  const t =
    length === 0
      ? 0
      : Math.max(0, Math.min(1, ((point[0] - from[0]) * dx + (point[2] - from[2]) * dz) / length));
  return Math.hypot(point[0] - from[0] - t * dx, point[2] - from[2] - t * dz);
}

const REACH = 0.075 * CITY;

/** The city's corners a node reaches, lit along the way to each it reaches: index to token. */
export function reachedCells(
  source: ColonyNode | undefined,
  reached: ReadonlySet<string>,
  nodes: readonly ColonyNode[],
): Map<number, string> {
  const cells = new Map<number, string>();
  if (!source) return cells;
  const targets = nodes.filter((n) => reached.has(n.id));
  LATTICE.points.forEach((point, i) => {
    if (gap(point, source.position) < 0.14 * CITY) cells.set(i, source.token);
    for (const t of targets) {
      if (offLine(point, source.position, t.position) < REACH) {
        cells.set(i, t.token);
        break;
      }
    }
  });
  return cells;
}

/** A name to place on screen. */
export interface Label {
  id: string;
  x: number;
  y: number;
  width: number;
  /** A label the human is looking at: never hidden. */
  pinned: boolean;
  /** Its depth: the nearer of two labels stays. */
  near: number;
}

/** The labels to hide: the farther of any two that touch, never one the human is looking at. */
export function crowdedLabels(labels: readonly Label[]): Set<string> {
  const ordered = [...labels].sort(
    (a, b) => Number(b.pinned) - Number(a.pinned) || b.near - a.near,
  );
  const kept: Label[] = [];
  const crowded = new Set<string>();
  for (const label of ordered) {
    const touches = kept.some(
      (k) => Math.abs(k.y - label.y) < 13 && Math.abs(k.x - label.x) < (k.width + label.width) / 2,
    );
    if (touches && !label.pinned) crowded.add(label.id);
    else kept.push(label);
  }
  return crowded;
}
