import { expect, test } from 'vitest';
import { parse } from '../router.ts';
import { commands, fuzzy, marked, QUERY_CAP, rank, SHOWN_CAP } from './commands.ts';
import { fixtureWorlds } from './fixtures.ts';
import { shortcuts } from './shortcuts.ts';

const worlds = fixtureWorlds.worlds;
const here = worlds[0]?.id;
const live = [
  {
    id: 'c-1',
    world: here ?? '',
    session: 's-1',
    run: 'r1-1',
    controlled: true,
    tabs: 1,
    asks: 1,
  },
];
const runs = [
  {
    id: 'r1-1',
    when: '2026-10-30T11:58:30Z',
    state: 'live',
    why: '',
    ask: 'fix the collate parse',
    turns: 1,
  },
  {
    id: 'r0-9',
    when: '2026-10-29T11:58:30Z',
    state: 'ended',
    why: '',
    ask: 'wire refactor',
    turns: 2,
  },
] as const;

test('the palette jumps to pages, worlds and their sections, and the chats and runs of the world it is in', () => {
  const all = commands(worlds, here, live as never, runs as never);
  const kinds = new Set(all.map((c) => c.kind));
  expect([...kinds].sort()).toEqual(['page', 'run', 'section', 'session', 'world']);
  for (const w of worlds)
    expect(all.some((c) => c.kind === 'world' && c.label === w.name)).toBe(true);
  expect(all.find((c) => c.kind === 'session')?.label).toBe('fix the collate parse');
  // A live chat's run is listed once, as the chat.
  expect(all.filter((c) => c.kind === 'run').map((c) => c.label)).toEqual(['wire refactor']);
  // Outside a world: no chats.
  expect(
    commands(worlds, undefined, live as never, runs as never).some((c) => c.kind === 'session'),
  ).toBe(false);
});

test('every command only moves the page to an address the board has: none answers or acts', () => {
  const all = commands(worlds, here, live as never, runs as never);
  for (const c of all) {
    expect(c.href.startsWith('#/'), c.href).toBe(true);
    expect(parse(c.href).name, c.href).not.toBe('unknown');
    expect(`${c.label} ${c.detail}`.toLowerCase()).not.toMatch(/approve|allow|yes\b|deny/);
  }
  const ids = all.map((c) => c.id);
  expect(new Set(ids).size).toBe(ids.length);
});

test('the fuzzy match: in order, case-insensitive, word starts and runs first', () => {
  expect(fuzzy('mem', 'Memory')?.at).toEqual([0, 1, 2]);
  expect(fuzzy('MEM', 'memory')).toBeDefined();
  expect(fuzzy('yrm', 'Memory')).toBeUndefined();
  expect(fuzzy('', 'Memory')).toEqual({ score: 0, at: [] });
  const run = fuzzy('ont', 'Ontology')?.score ?? 0;
  const gaps = fuzzy('ont', 'one night town')?.score ?? 0;
  expect(run).toBeGreaterThan(gaps);
  // Code points, not code units: an astral character is one place.
  expect(fuzzy('b', '𝒜b')?.at).toEqual([1]);
});

test('a query is matched by a scan, never compiled: regex syntax is just text', () => {
  expect(fuzzy('.*', 'Memory')).toBeUndefined();
  expect(fuzzy('(a+)+$', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa!')).toBeUndefined();
  expect(fuzzy('a.b', 'a.b')?.at).toEqual([0, 1, 2]);
});

test('the match stays linear on hostile input', () => {
  const text = 'a'.repeat(200_000);
  const query = `${'a'.repeat(QUERY_CAP - 1)}b`;
  const t0 = performance.now();
  for (let i = 0; i < 20; i++) expect(fuzzy(query, text)).toBeUndefined();
  expect(performance.now() - t0).toBeLessThan(2000);
});

test('the ranking: best first, the detail counting too, capped', () => {
  const all = commands(worlds, here, live as never, runs as never);
  const sql = worlds.find((w) => w.name === 'sqlglot');
  const top = rank(all, 'memory sqlglot')[0]?.command;
  expect(top?.label).toBe('Memory');
  expect(top?.detail).toBe(sql?.name);
  expect(rank(all, 'metrics')[0]?.command.label).toBe('Metrics');
  expect(rank(all, '').length).toBeLessThanOrEqual(SHOWN_CAP);
  expect(rank(all, 'zzzzqqq')).toEqual([]);
});

test('marked cuts a label at its matches', () => {
  expect(marked('Memory', [0, 2])).toEqual([
    { text: 'M', hit: true },
    { text: 'e', hit: false },
    { text: 'm', hit: true },
    { text: 'ory', hit: false },
  ]);
});

test('the shortcut map lists the palette and itself, and no shortcut approves', () => {
  const all = shortcuts('Ctrl').flatMap((g) => g.items);
  expect(all.some((s) => s.keys.join('+') === 'Ctrl+K')).toBe(true);
  expect(all.some((s) => s.keys.join('+') === '?')).toBe(true);
  for (const s of all) expect(s.does.toLowerCase()).not.toMatch(/approv|allow|\byes\b/);
});
