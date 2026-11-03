import type { BoardBody } from '@mnemorph/protocol';
import { describe, expect, it } from 'vitest';
import { parseBodies } from './bodies.ts';
import { fixtureBodies, fixtureRaces } from './bodiesFixtures.ts';
import { colony, LANES, NODE_W, sortLanes, THRONE } from './colony.ts';

const idle = { state: 'idle' as const, why: '', run: null, at: null };

function b(name: string, race: string, parent: string | null = null): BoardBody {
  const lane = fixtureRaces.find((r) => r.race === race)?.base ?? race;
  return {
    name,
    race,
    lane,
    parent,
    post: null,
    speciality: null,
    ground: [],
    verify: 0,
    retired: false,
    heir_of: null,
    doc: '',
    minds: [],
    live: idle,
    acts: [],
    standing: 'trusted',
    runs_as: race,
    differs: [],
    proposal: null,
    probation: null,
  };
}

const throne = { live: false, why: '' };

/** Whether `edges` hold a cycle. */
function cyclic(edges: { from: string; to: string }[]): boolean {
  const up = new Map(edges.map((e) => [e.to, e.from]));
  for (const start of up.keys()) {
    const seen = new Set<string>();
    for (let at: string | undefined = start; at !== undefined; at = up.get(at)) {
      if (seen.has(at)) return true;
      seen.add(at);
    }
  }
  return false;
}

describe('the colony model', () => {
  it('sorts lanes by race, whatever the order bodies come in', () => {
    const bodies = [
      b('dark-elf-supply', 'dark-elf'),
      b('slime-a', 'named-slime', 'orc-a'),
      b('kijin-forge', 'oni'),
      b('orc-a', 'orc', 'elf-a'),
      b('elf-a', 'high-elf'),
    ];
    const c = colony(bodies, fixtureRaces, throne);
    expect(c.lanes.map((l) => l.race)).toEqual([
      'rimuru',
      'elf',
      'orc',
      'slime',
      'kijin',
      'dark-elf',
    ]);
    expect(sortLanes(['zeta', 'slime', 'rimuru', 'alpha'])).toEqual([
      'rimuru',
      'slime',
      'alpha',
      'zeta',
    ]);
    // An ascended form sits in its base race's lane.
    expect(c.nodes.find((n) => n.name === 'slime-a')?.lane).toBe('slime');
    // Lanes go down the page in order, and no two nodes share a place.
    const ys = c.lanes.map((l) => l.y);
    expect([...ys].sort((x, y) => x - y)).toEqual(ys);
    const places = new Set(c.nodes.map((n) => `${n.x},${n.y}`));
    expect(places.size).toBe(c.nodes.length);
    expect(Math.max(...c.nodes.map((n) => n.x + NODE_W / 2))).toBeLessThanOrEqual(c.width);
  });

  it('draws the command tree: Reports to, and the throne over its races', () => {
    const page = parseBodies(fixtureBodies);
    if (!page) throw new Error('the fixture reads');
    const c = colony(page.bodies, page.races, page.throne);
    const edges = c.edges.map((e) => `${e.from}>${e.to}`).sort();
    expect(edges).toEqual(
      [
        'rimuru>elf-sql',
        'elf-sql>orc-sql',
        'orc-sql>slime-parser',
        'orc-sql>slime-lexer',
        'orc-sql>slime-‮evil',
        'rimuru>kijin-forge',
        'rimuru>dark-elf-supply',
      ].sort(),
    );
    // A parent no body has is said, never drawn.
    expect(c.dropped).toEqual([{ from: 'orc-gone', to: 'slime-old', why: 'unknown' }]);
    // A child sits under its commander's column order: the parser and lexer after the orc.
    expect(c.lanes.find((l) => l.race === 'slime')?.nodes[0]).not.toBe('slime-old');
  });

  it('draws the throne itself when no doc describes it', () => {
    const c = colony([b('elf-a', 'elf'), b('kijin-ci', 'kijin')], fixtureRaces, {
      live: true,
      why: '',
    });
    const t = c.nodes.find((n) => n.name === THRONE);
    expect(t?.synthetic).toBe(true);
    expect(t?.state).toBe('working');
    expect(c.edges).toEqual([
      { from: THRONE, to: 'elf-a' },
      { from: THRONE, to: 'kijin-ci' },
    ]);
  });

  it('can never draw a cycle', () => {
    const loop = [
      b('orc-a', 'orc', 'orc-b'),
      b('orc-b', 'orc', 'orc-c'),
      b('orc-c', 'orc', 'orc-a'),
    ];
    const c = colony(loop, fixtureRaces, throne);
    expect(cyclic(c.edges)).toBe(false);
    expect(c.dropped.map((d) => d.why)).toEqual(['loop']);
    const self = colony([b('orc-a', 'orc', 'orc-a')], fixtureRaces, throne);
    expect(self.edges).toEqual([]);
    expect(self.dropped).toEqual([{ from: 'orc-a', to: 'orc-a', why: 'loop' }]);
    // Any doc's parents, at random: the drawing stays a forest.
    let seed = 7;
    const rand = (n: number) => {
      seed = (seed * 1103515245 + 12345) % 2147483648;
      return seed % n;
    };
    for (let round = 0; round < 200; round++) {
      const n = 2 + rand(12);
      const races = ['orc', 'slime', 'elf', 'kijin', 'rimuru'];
      const bodies = Array.from({ length: n }, (_, i) =>
        b(`c${i}`, races[rand(races.length)] ?? 'orc', rand(4) === 0 ? null : `c${rand(n + 1)}`),
      );
      const g = colony(bodies, fixtureRaces, throne);
      expect(cyclic(g.edges)).toBe(false);
      // Every parent a doc names is drawn, or said dropped.
      for (const x of bodies.filter((x) => x.parent !== null)) {
        const drawn = g.edges.some((e) => e.to === x.name && e.from === x.parent);
        const said = g.dropped.some((d) => d.to === x.name && d.from === x.parent);
        expect(drawn || said).toBe(true);
      }
    }
  });

  it('labels every node in words, never colour alone', () => {
    const page = parseBodies(fixtureBodies);
    if (!page) throw new Error('the fixture reads');
    const c = colony(page.bodies, page.races, page.throne);
    expect(c.nodes.length).toBe(page.bodies.length);
    for (const n of c.nodes) {
      expect(n.label).toContain(n.name);
      expect(n.label).toContain(n.title);
      expect(n.label.split(' · ').length).toBeGreaterThanOrEqual(3);
    }
    expect(c.nodes.find((n) => n.name === 'kijin-forge')?.label).toBe(
      'kijin-forge · Kijin · post forge · not commissioned yet',
    );
    expect(LANES[0]).toBe('rimuru');
  });
});
