import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { hidden, runs, shown } from './shown.ts';

/** The vectors the Rust tests read too (crates/mnemorph-loop/tests/shown_vectors.rs). */
const vectors = JSON.parse(
  readFileSync(
    new URL('../../../../crates/mnemorph-loop/tests/shown-vectors.json', import.meta.url),
    'utf8',
  ),
) as { hidden: { cp: string; hidden: boolean }[]; shown: { in: string; out: string }[] };

describe('the shared vectors', () => {
  it('hold enough cases to mean something', () => {
    expect(vectors.hidden.length).toBeGreaterThan(20);
    expect(vectors.shown.length).toBeGreaterThan(20);
  });
  for (const v of vectors.hidden) {
    it(`U+${v.cp.toUpperCase()} is ${v.hidden ? '' : 'not '}hidden`, () =>
      expect(hidden(Number.parseInt(v.cp, 16))).toBe(v.hidden));
  }
  for (const v of vectors.shown) {
    it(`shown(${JSON.stringify(v.in)})`, () => expect(shown(v.in)).toBe(v.out));
  }
});

describe('beyond Rust', () => {
  it('escapes a lone surrogate, which only a JavaScript string can hold', () => {
    expect(shown('a\ud800b')).toBe('a\\u{d800}b');
  });
  it('splits text into runs, escapes apart', () => {
    expect(runs('a\u202eb')).toEqual([
      { text: 'a', escaped: false },
      { text: '\\u{202e}', escaped: true },
      { text: 'b', escaped: false },
    ]);
  });
  it('keeps newlines as written in prose, and escapes the rest', () => {
    expect(runs('one\ntwo\r', true).map((r) => r.text)).toEqual(['one\ntwo', '\\u{d}']);
    expect(runs('one\ntwo').map((r) => r.text)).toEqual(['one', '\\u{a}', 'two']);
  });
  it('joins its runs into exactly shown', () => {
    for (const v of vectors.shown) {
      expect(
        runs(v.in)
          .map((r) => r.text)
          .join(''),
      ).toBe(v.out);
    }
  });
});
