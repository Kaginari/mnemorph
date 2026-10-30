import { describe, expect, it } from 'vitest';
import { ARM_MS, actRows, askDigest, Card, CONFIRM_MS, escapes } from './approval.ts';

/** A card of `rows` rows, visible and acknowledged at 0: armed at 300. */
function armed(rows = 1): Card {
  const c = new Card(rows);
  c.visible(true, 0);
  c.acked(true, 0);
  for (let i = 0; i < rows; i++) c.rowSeen(i);
  return c;
}

describe('the act in rows', () => {
  it('ends a row after each escaped newline and never cuts an escape', () => {
    expect(actRows('ls\\u{a}rm x')).toEqual(['ls\\u{a}', 'rm x']);
    const long = `${'a'.repeat(62)}\\u{202e}b`;
    const rows = actRows(long);
    expect(rows.join('')).toBe(long);
    expect(rows[0]).toBe('a'.repeat(62));
    expect(rows[1]).toBe('\\u{202e}b');
    expect(actRows('')).toEqual(['']);
  });

  it('sets the escapes apart', () => {
    expect(escapes('a\\u{1b}[2J')).toEqual([
      { text: 'a', escaped: false },
      { text: '\\u{1b}', escaped: true },
      { text: '[2J', escaped: false },
    ]);
  });
});

it('computes the digest the board does (the shared vector)', async () => {
  const d = await askDigest('gate-1', {
    body: 'rimuru',
    race: 'rimuru',
    class: 'write',
    act: 'rm -rf build\n\\u{202e} é',
    why: ['writes', 'deletes'],
    outside: [],
    plan: 'plan 3f9a',
  });
  expect(d).toBe('2d1d901cda4148560ddd9c4fe7c414a6ffe07a70c873ed9045a5c1c8e095b4d1');
});

describe('the card', () => {
  it('ignores a key before it is armed, and does not arm while hidden or unacknowledged', () => {
    const c = new Card(1);
    c.rowSeen(0);
    c.visible(true, 0);
    expect(c.armed(1000)).toBe(false);
    c.acked(true, 0);
    expect(c.key('y', ARM_MS - 1, 1)).toEqual({ kind: 'none' });
    expect(c.choice).toBeUndefined();
    expect(c.armed(ARM_MS)).toBe(true);
    c.visible(false, 400);
    expect(c.armed(10_000)).toBe(false);
    c.visible(true, 1000);
    expect(c.armed(1000 + ARM_MS - 1)).toBe(false);
    expect(c.armed(1000 + ARM_MS)).toBe(true);
  });

  it('approves on y, then Enter in another task past the confirmation', () => {
    const c = armed();
    c.key('y', 400, 1);
    expect(c.key('Enter', 400 + CONFIRM_MS, 2)).toEqual({ kind: 'allow' });
  });

  it('is disarmed by y and Enter in one task, or too close together', () => {
    const c = armed();
    c.key('y', 400, 1);
    expect(c.key('Enter', 1000, 1)).toEqual({ kind: 'none' });
    expect(c.choice).toBeUndefined();
    expect(c.armed(1000 + ARM_MS - 1)).toBe(false);
    c.key('y', 2000, 3);
    expect(c.key('Enter', 2000 + CONFIRM_MS - 1, 4)).toEqual({ kind: 'none' });
    expect(c.armed(2100)).toBe(false);
  });

  it('is disarmed by any other key and by a paste: a typed "yes" approves nothing', () => {
    const c = armed();
    c.key('y', 400, 1);
    c.key('e', 450, 2);
    expect(c.choice).toBeUndefined();
    c.key('s', 500, 3);
    expect(c.key('Enter', 900, 4)).toEqual({ kind: 'none' });
    const d = armed();
    d.key('y', 400, 1);
    d.pasted(500);
    expect(d.key('Enter', 700, 2)).toEqual({ kind: 'none' });
    expect(d.armed(799)).toBe(false);
  });

  it('refuses yes until every row was read in order', () => {
    const c = new Card(3);
    c.visible(true, 0);
    c.acked(true, 0);
    c.rowSeen(0);
    c.rowSeen(2);
    expect(c.seenTo).toBe(1);
    c.key('y', 400, 1);
    expect(c.unread).toBe(true);
    expect(c.key('Enter', 700, 2)).toEqual({ kind: 'none' });
    expect(c.click(0, 800)).toEqual({ kind: 'none' });
    c.rowSeen(1);
    c.rowSeen(2);
    expect(c.viewed).toBe(true);
    expect(c.click(0, 900)).toEqual({ kind: 'allow' });
  });

  it('denies on Esc, armed or not; picks no and no-with-a-reason', () => {
    expect(new Card(1).key('Escape', 0, 0)).toEqual({ kind: 'deny' });
    const c = armed();
    c.key('2', 400, 1);
    expect(c.key('Enter', 600, 2)).toEqual({ kind: 'deny' });
    const d = armed();
    d.key('3', 400, 1);
    expect(d.key('Enter', 600, 2)).toEqual({ kind: 'reason' });
  });

  it('scrolls and moves focus without disarming', () => {
    const c = armed();
    c.key('y', 400, 1);
    for (const k of ['ArrowDown', 'PageDown', ' ', 'Tab', 'Shift']) c.key(k, 450, 2);
    expect(c.key('Enter', 700, 3)).toEqual({ kind: 'allow' });
  });

  it('starts again when control moves', () => {
    const c = armed();
    c.acked(false, 500);
    expect(c.armed(5000)).toBe(false);
    c.acked(true, 5000);
    expect(c.armed(5000 + ARM_MS - 1)).toBe(false);
  });
});
