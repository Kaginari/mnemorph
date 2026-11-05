import { type RaceKey, races } from '@mnemorph/theme';
import { describe, expect, test } from 'vitest';
import {
  baseOf,
  colour,
  DEFAULT_CAMERA,
  EDGES,
  CITY,
  crowdedLabels,
  energy,
  FOV,
  LATTICE,
  MAX_PITCH,
  MAX_ZOOM,
  MIN_PITCH,
  MIN_ZOOM,
  NODES,
  nearest,
  netOf,
  node,
  orbit,
  paintOrder,
  project,
  raceOf,
  reachedCells,
  related,
  rotate,
  scaleAt,
  spin,
  terrace,
  toScreen,
  zoomed,
} from './net.ts';

/** A node the layout must have. */
function must(id: string) {
  const n = node(id);
  if (!n) throw new Error(`no node ${id}`);
  return n;
}

describe('the colony graph', () => {
  test('holds every race and ascended form, ids unique', () => {
    expect(NODES).toHaveLength(12);
    expect(new Set(NODES.map((n) => n.id)).size).toBe(NODES.length);
    const ids = [
      'veldora',
      'rimuru',
      'elf',
      'orc',
      'slime',
      'kijin',
      'darkElf',
      'highElf',
      'highOrc',
      'namedSlime',
      'oni',
      'archDarkElf',
    ];
    for (const id of ids) {
      expect(node(id), id).toBeDefined();
    }
  });

  test('the hierarchy: the throne over the base races, the chain, the ascensions', () => {
    const command = EDGES.filter((e) => e.kind === 'command')
      .map((e) => `${e.from}->${e.to}`)
      .sort();
    expect(command).toEqual(
      [
        'elf->orc',
        'orc->slime',
        'rimuru->darkElf',
        'rimuru->elf',
        'rimuru->kijin',
        'veldora->rimuru',
      ].sort(),
    );
    const ascends = EDGES.filter((e) => e.kind === 'ascends')
      .map((e) => `${e.from}->${e.to}`)
      .sort();
    expect(ascends).toEqual(
      [
        'elf->highElf',
        'orc->highOrc',
        'slime->namedSlime',
        'kijin->oni',
        'darkElf->archDarkElf',
      ].sort(),
    );
  });

  test('every edge names existing nodes; a base is built, its ascended form rises from it', () => {
    const ids = new Set(NODES.map((n) => n.id));
    for (const e of EDGES) {
      expect(ids.has(e.from), `${e.from} exists`).toBe(true);
      expect(ids.has(e.to), `${e.to} exists`).toBe(true);
    }
    for (const n of NODES.filter((n) => n.kind === 'ascended')) {
      const base = baseOf(n);
      expect(base.kind).toBe('base');
      expect(EDGES.some((e) => e.kind === 'ascends' && e.from === base.id && e.to === n.id)).toBe(
        true,
      );
    }
    for (const n of NODES.filter((n) => n.kind === 'base')) {
      expect(
        EDGES.some((e) => e.to === n.id),
        `${n.id} is commanded`,
      ).toBe(true);
    }
  });

  test("each node wears its race's token, and the theme has that race", () => {
    for (const n of NODES) {
      expect(Object.keys(races), n.id).toContain(n.id as RaceKey);
      const kebab = n.id.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
      expect(n.token).toBe(`--mn-race-${kebab}`);
      expect(colour(n)).toBe(`var(${n.token})`);
    }
  });
});

describe('the camera', () => {
  test('orbit changes yaw freely and clamps pitch', () => {
    const c = orbit(DEFAULT_CAMERA, 1, 9);
    expect(c.yaw).toBeCloseTo(DEFAULT_CAMERA.yaw + 1);
    expect(c.pitch).toBe(MAX_PITCH);
    expect(orbit(c, 0, -9).pitch).toBe(MIN_PITCH);
  });

  test('zoom scales and clamps', () => {
    expect(zoomed(DEFAULT_CAMERA, 2).zoom).toBeCloseTo(2);
    expect(zoomed(DEFAULT_CAMERA, 0.0001).zoom).toBe(MIN_ZOOM);
    expect(zoomed(DEFAULT_CAMERA, 1e6).zoom).toBe(MAX_ZOOM);
  });

  test('spin advances yaw only when allowed', () => {
    expect(spin(DEFAULT_CAMERA, 2, false)).toEqual(DEFAULT_CAMERA);
    expect(spin(DEFAULT_CAMERA, 2, true).yaw).toBeCloseTo(DEFAULT_CAMERA.yaw + 0.5);
  });
});

describe('the projection', () => {
  test('the origin projects to the screen centre with unit scale', () => {
    expect(rotate([0, 0, 0], DEFAULT_CAMERA)).toEqual({ x: 0, y: 0, z: 0 });
    expect(scaleAt(0)).toBe(1);
    const s = toScreen([0, 0, 0], DEFAULT_CAMERA, 1200, 800);
    expect(s.sx).toBe(600);
    expect(s.sy).toBe(400);
  });

  test('the throne stands on the centre line, above the base races', () => {
    const level = { yaw: 0, pitch: 0, zoom: 1 };
    const throne = toScreen(must('rimuru').position, level, 1200, 800);
    expect(throne.sx).toBeCloseTo(600);
    expect(throne.sy).toBeLessThan(toScreen(must('orc').position, level, 1200, 800).sy);
  });

  test('a half-turn mirrors x for a point on the depth plane', () => {
    const level = { yaw: 0, pitch: 0, zoom: 1 };
    const a = toScreen([2, 1, 0], level, 1000, 800);
    const b = toScreen([2, 1, 0], { ...level, yaw: Math.PI }, 1000, 800);
    expect(a.sx).toBeCloseTo(1000 - b.sx);
    expect(a.sy).toBeCloseTo(b.sy);
  });

  test('nearer projects bigger', () => {
    const points = NODES.map((n) => project(n, DEFAULT_CAMERA, 1000, 800));
    const byDepth = paintOrder(points);
    const far = byDepth[0];
    const near = byDepth[byDepth.length - 1];
    expect(far && near && far.depth < near.depth).toBe(true);
    expect(near?.radius).toBeGreaterThan(far?.radius ?? Number.POSITIVE_INFINITY);
    expect(scaleAt(1)).toBeGreaterThan(scaleAt(-1));
  });

  test('from every camera, no node shrinks below 0.6 or swells past twice its size', () => {
    let lo = Number.POSITIVE_INFINITY;
    let hi = 0;
    for (let yaw = 0; yaw < 2 * Math.PI; yaw += 0.1) {
      for (let pitch = MIN_PITCH; pitch <= MAX_PITCH; pitch += 0.05) {
        for (const n of NODES) {
          const s = scaleAt(rotate(n.position, { yaw, pitch, zoom: 1 }).z);
          lo = Math.min(lo, s);
          hi = Math.max(hi, s);
        }
      }
    }
    expect(lo).toBeGreaterThan(0.6);
    expect(hi).toBeLessThan(2);
  });

  test('at the default camera no two nodes overlap on screen', () => {
    const points = NODES.map((n) => project(n, DEFAULT_CAMERA, 1000, 800));
    for (const [i, a] of points.entries()) {
      for (const b of points.slice(i + 1)) {
        const gap = Math.hypot(a.x - b.x, a.y - b.y) - a.radius - b.radius;
        expect(gap, `${a.node.id} and ${b.node.id}`).toBeGreaterThan(0);
      }
    }
  });

  test('paint order is far to near', () => {
    const points = NODES.map((n) => project(n, DEFAULT_CAMERA, 1000, 800));
    const ordered = paintOrder(points);
    const depths = ordered.map((pl) => pl.depth);
    for (let i = 1; i < depths.length; i++) {
      expect(depths[i - 1] ?? Number.NaN).toBeLessThanOrEqual(depths[i] ?? Number.NaN);
    }
    expect(ordered[0]?.node.id).toBe(points.reduce((a, b) => (a.depth < b.depth ? a : b)).node.id);
  });

  test('nearest picks the closest node within reach and misses far away', () => {
    const points = NODES.map((n) => project(n, DEFAULT_CAMERA, 1000, 800));
    const rimuru = project(must('rimuru'), DEFAULT_CAMERA, 1000, 800);
    expect(nearest(points, rimuru.x + 3, rimuru.y + 3, 30)?.node.id).toBe('rimuru');
    expect(nearest(points, 0, 0, 1)).toBeUndefined();
    expect(FOV).toBeGreaterThan(0);
  });
});

describe('the city', () => {
  test('the terrain peaks at the throne and falls to the rim', () => {
    expect(terrace(0, 0)).toBeGreaterThan(terrace(0.5, 0));
    expect(terrace(0.5, 0)).toBeGreaterThan(terrace(1, 0));
    expect(terrace(3, 3)).toBe(terrace(1, 0));
  });

  test('the lattice is one city: each corner shared, every link between corners it holds', () => {
    const { links, points } = LATTICE;
    expect(points.length).toBeGreaterThan(400);
    expect(links.length).toBeGreaterThan(points.length);
    expect(links.every(([a, b]) => a !== b && a < points.length && b < points.length)).toBe(true);
    expect(new Set(points.map((pt) => pt.join())).size).toBe(points.length);
    const reach = Math.max(...points.map(([x, , z]) => Math.hypot(x, z)));
    expect(reach).toBeLessThanOrEqual(CITY * 1.06);
  });

  test('a wave lights the ring it has reached and is gone when it ends', () => {
    const wave = { origin: [0, 0, 0] as const, start: 0 };
    const ring = 1.5 * CITY;
    expect(energy([ring, 0, 0], [wave], 1000).power).toBeGreaterThan(0.4);
    expect(energy([0, 0, 0], [wave], 1000).power).toBe(0);
    expect(energy([ring, 0, 0], [wave], 3000).power).toBe(0);
    expect(energy([ring, 0, 0], [], 1000).power).toBe(0);
  });

  test('each living body is seated around its race; a retired one or an unknown race is not', () => {
    const body = (name: string, race: string, retired = false, working = false) => ({
      name,
      race,
      retired,
      working,
    });
    const net = netOf([
      body('slime-parser', 'slime', false, true),
      body('slime-old', 'slime', true),
      body('orc-law', 'high-orc'),
      body('rimuru', 'rimuru'),
      body('stranger', 'no-such-race'),
    ]);
    const seated = net.nodes.filter((n) => n.kind === 'body');
    expect(seated.map((n) => n.name)).toEqual(['slime-parser', 'orc-law']);
    expect(net.edges.filter((e) => e.kind === 'seat')).toEqual([
      { from: 'slime', to: 'body:slime-parser', kind: 'seat' },
      { from: 'highOrc', to: 'body:orc-law', kind: 'seat' },
    ]);
    expect([...net.working]).toEqual(['body:slime-parser']);
    expect(seated[0]?.token).toBe(must('slime').token);
    expect(raceOf(must('darkElf'))).toBe('dark-elf');
  });

  test('a crowded race seats a dozen, and bodies stay on the map', () => {
    const many = Array.from({ length: 40 }, (_, i) => ({
      name: `slime-${i}`,
      race: 'slime',
      retired: false,
      working: false,
    }));
    const seated = netOf(many).nodes.filter((n) => n.kind === 'body');
    expect(seated).toHaveLength(12);
    for (const n of seated) expect(scaleAt(rotate(n.position, DEFAULT_CAMERA).z)).toBeLessThan(2);
  });

  test('a pick reaches who it commissions and who commissions it, and lights the way there', () => {
    const elf = must('elf');
    const near = related('elf', EDGES);
    expect([...near].sort()).toEqual(['highElf', 'orc', 'rimuru']);
    expect(related(undefined, EDGES).size).toBe(0);
    const cells = reachedCells(elf, near, NODES);
    expect(cells.size).toBeGreaterThan(10);
    expect(new Set(cells.values()).has(must('orc').token)).toBe(true);
    expect(reachedCells(undefined, near, NODES).size).toBe(0);
  });

  test('the farther of two labels that touch goes, never one being looked at', () => {
    const label = (id: string, x: number, near: number, pinned = false) => ({
      id,
      x,
      y: 100,
      width: 40,
      near,
      pinned,
    });
    expect([...crowdedLabels([label('back', 100, 0), label('front', 110, 1)])]).toEqual(['back']);
    expect(crowdedLabels([label('a', 100, 0), label('b', 200, 1)]).size).toBe(0);
    expect([...crowdedLabels([label('picked', 100, 0, true), label('front', 110, 1)])]).toEqual([
      'front',
    ]);
  });
});
