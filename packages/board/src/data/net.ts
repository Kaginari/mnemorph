/**
 * The colony net page's model (`#/net`): the six races and their five ascended forms drawn as a
 * neural network — the colony's shape, the same in every world. A world's own bodies, live, are
 * its Bodies page (`data/colony.ts`).
 *
 * Pure layout and 3D geometry only; the canvas (`views/Net.tsx`) keeps no logic a test could
 * reach. jsdom has no canvas, so everything here runs under plain vitest.
 *
 * The colony's shape (docs/02-races.md): Veldora (the human) over the throne, Rimuru over the
 * base races — the chain Elf → Orc → Slime across the front, Kijin and Dark Elf standing
 * beside — and each base's ascended form floating above it, joined by a dashed ascension edge.
 */

export type NodeKind = 'human' | 'throne' | 'base' | 'ascended';

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
  /** command: who commissions; ascends: who rose from whom. */
  kind: 'command' | 'ascends';
}

const p = (x: number, y: number, z: number): readonly [number, number, number] => [x, y, z];

export const NODES: readonly ColonyNode[] = [
  {
    id: 'veldora',
    name: 'Veldora',
    job: 'the human — everything answers to you',
    token: '--mn-race-veldora',
    position: p(0, 3.7, -2.2),
    kind: 'human',
  },
  {
    id: 'rimuru',
    name: 'Rimuru',
    job: 'understand the human, plan, answer',
    token: '--mn-race-rimuru',
    position: p(0, 2.5, -1),
    kind: 'throne',
  },
  {
    id: 'elf',
    name: 'Elf',
    job: 'plan one lane of work across domains',
    token: '--mn-race-elf',
    position: p(-2.6, 0, 0.2),
    kind: 'base',
  },
  {
    id: 'orc',
    name: 'Orc',
    job: 'own a domain — the test first, judge the landing',
    token: '--mn-race-orc',
    position: p(0, 0, 0.2),
    kind: 'base',
  },
  {
    id: 'slime',
    name: 'Slime',
    job: 'change the code of its one zone, with its doc',
    token: '--mn-race-slime',
    position: p(2.6, 0, 0.2),
    kind: 'base',
  },
  {
    id: 'kijin',
    name: 'Kijin',
    job: 'everything that leaves the machine — CI, releases, deploys',
    token: '--mn-race-kijin',
    position: p(-1.5, -0.3, 1.9),
    kind: 'base',
  },
  {
    id: 'darkElf',
    name: 'Dark Elf',
    job: 'attack the code; prove every weakness it claims',
    token: '--mn-race-dark-elf',
    position: p(1.5, -0.3, 1.9),
    kind: 'base',
  },
  {
    id: 'highElf',
    name: 'High Elf',
    job: 'the Elf ascended — the same lane, less friction',
    token: '--mn-race-high-elf',
    position: p(-2.6, 1.3, -0.1),
    kind: 'ascended',
    ascendsFrom: 'elf',
  },
  {
    id: 'highOrc',
    name: 'High Orc',
    job: 'the Orc ascended — the same domain, less friction',
    token: '--mn-race-high-orc',
    position: p(0, 1.3, -0.1),
    kind: 'ascended',
    ascendsFrom: 'orc',
  },
  {
    id: 'namedSlime',
    name: 'Named Slime',
    job: 'the Slime ascended — the same zone, less friction',
    token: '--mn-race-named-slime',
    position: p(2.6, 1.3, -0.1),
    kind: 'ascended',
    ascendsFrom: 'slime',
  },
  {
    id: 'oni',
    name: 'Oni',
    job: 'the Kijin ascended — the same reach, less friction',
    token: '--mn-race-oni',
    position: p(-1.5, 1, 1.6),
    kind: 'ascended',
    ascendsFrom: 'kijin',
  },
  {
    id: 'archDarkElf',
    name: 'Arch Dark Elf',
    job: 'the Dark Elf ascended — the same ground, less friction',
    token: '--mn-race-arch-dark-elf',
    position: p(1.5, 1, 1.6),
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

export const DEFAULT_CAMERA: Camera = { yaw: 0.55, pitch: 0.22, zoom: 1 };
export const MIN_ZOOM = 0.45;
export const MAX_ZOOM = 3;
export const MIN_PITCH = -1.25;
export const MAX_PITCH = 1.25;
export const FOV = 9;
export const NODE_RADIUS = 0.3;

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
  const unit = (height / 10) * camera.zoom;
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
    radius: Math.max(NODE_RADIUS * s.scale * (height / 10) * camera.zoom, 1.5),
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

/** The faint floor grid under the net, as polylines of points. */
export const GRID: readonly (readonly [number, number, number])[][] = (() => {
  const n = 7;
  const ticks = Array.from({ length: n }, (_, i) => -4.2 + i * 1.4);
  const lines: (readonly [number, number, number])[][] = [];
  const y = -1.05;
  for (const z of ticks) lines.push(ticks.map((x) => p(x, y, z)));
  for (const x of ticks) lines.push(ticks.map((z) => p(x, y, z)));
  return lines;
})();
