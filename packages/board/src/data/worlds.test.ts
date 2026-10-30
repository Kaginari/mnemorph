import { describe, expect, it } from 'vitest';
import { fixtureWorlds } from './fixtures.ts';
import { parseWorlds } from './worlds.ts';

/**
 * `GET /api/worlds` byte for byte as the board server's handler writes it: serde's compact JSON
 * of `BoardWorlds`, fields in declaration order, `null` for an absent remote or a world never
 * opened (crates/mnemorph-server/src/board/mod.rs, tests/board.rs).
 */
const SERVED =
  '{"worlds":[' +
  '{"id":"alpha-1f2e3d4c5b6a7980","name":"alpha","path":"/home/u/alpha","remote":"github.com/o/alpha","last":"2026-10-30T19:20:11Z","sessions":3,"state":"present"},' +
  '{"id":"gone-0a1b2c3d4e5f6071","name":"gone","path":"/home/u/gone","remote":null,"last":"2026-10-29T08:00:00Z","sessions":1,"state":"missing"},' +
  '{"id":"beta-99aa88bb77cc66dd","name":"beta","path":"/home/u/beta","remote":null,"last":null,"sessions":0,"state":"present"}' +
  ']}';

describe('the worlds answer', () => {
  it("reads the server's own answer whole, nulls included", () => {
    const { worlds, unreadable } = parseWorlds(JSON.parse(SERVED));
    expect(unreadable).toBe(0);
    expect(worlds.map((w) => w.name)).toEqual(['alpha', 'gone', 'beta']);
    expect(worlds[1]).toEqual({
      id: 'gone-0a1b2c3d4e5f6071',
      name: 'gone',
      path: '/home/u/gone',
      remote: null,
      last: '2026-10-29T08:00:00Z',
      sessions: 1,
      state: 'missing',
    });
    expect(worlds[2]?.last).toBeNull();
  });

  it('sorts the latest session first, a world never opened last', () => {
    const { worlds, unreadable } = parseWorlds(fixtureWorlds);
    expect(unreadable).toBe(0);
    expect(worlds.map((w) => w.name)).toEqual(['tempest-try', 'sqlglot', 'old-spike', 'notes']);
  });

  it('leaves out and counts a row that does not hold together', () => {
    const good = fixtureWorlds.worlds[0];
    const { worlds, unreadable } = parseWorlds({
      worlds: [
        good,
        null,
        { ...good, id: '' },
        { ...good, state: 'live' },
        { ...good, sessions: -1 },
        { ...good, sessions: 1.5 },
        { ...good, remote: 7 },
        { ...good, remote: undefined },
        { ...good, last: 'yesterday' },
        { ...good, path: undefined },
        { ...good, name: undefined },
      ],
    });
    expect(worlds).toHaveLength(1);
    expect(unreadable).toBe(10);
  });

  it('keeps no field it does not know', () => {
    const good = fixtureWorlds.worlds[0];
    const { worlds } = parseWorlds({ worlds: [{ ...good, html: '<b>x</b>' }] });
    expect(worlds[0]).not.toHaveProperty('html');
  });

  it.each([null, 3, 'worlds', {}, { worlds: {} }])('refuses %j', (body) => {
    expect(() => parseWorlds(body)).toThrow('no list of worlds');
  });
});
