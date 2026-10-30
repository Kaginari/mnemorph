import { describe, expect, it } from 'vitest';
import { fixtureWorlds } from './fixtures.ts';
import { parseWorlds } from './worlds.ts';

describe('the worlds answer', () => {
  it('reads the fixtures whole, live first, then latest first', () => {
    const { worlds, unreadable } = parseWorlds(fixtureWorlds);
    expect(unreadable).toBe(0);
    expect(worlds.map((w) => w.status)).toEqual(['live', 'tui-live', 'idle', 'missing']);
  });

  it('leaves out and counts a row that does not hold together', () => {
    const good = fixtureWorlds.worlds[0];
    const { worlds, unreadable } = parseWorlds({
      worlds: [
        good,
        null,
        { ...good, id: '' },
        { ...good, status: 'haunted' },
        { ...good, sessions: -1 },
        { ...good, sessions: 1.5 },
        { ...good, remote: 7 },
        { ...good, name: undefined },
      ],
    });
    expect(worlds).toHaveLength(1);
    expect(unreadable).toBe(7);
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
