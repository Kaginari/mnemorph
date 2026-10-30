import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { HIDDEN, hidden, keptInProse, runs, shown } from './shown.ts';

/** The vectors the Rust tests read too (crates/mnemorph-loop/tests/shown_vectors.rs). */
const vectors = JSON.parse(
  readFileSync(
    new URL('../../../../crates/mnemorph-loop/tests/shown-vectors.json', import.meta.url),
    'utf8',
  ),
) as {
  escaped: { from: string; to: string; why: string }[];
  prose: string[];
  hidden: { cp: string; hidden: boolean }[];
  shown: { in: string; out: string }[];
};

const hex = (s: string) => Number.parseInt(s, 16);
const listed = vectors.escaped.map((r) => [hex(r.from), hex(r.to)] as const);
const surrogate = (cp: number) => cp >= 0xd800 && cp <= 0xdfff;

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

describe('the shared rule', () => {
  it('is the ranges the vectors list', () => {
    expect(HIDDEN.map(([a, b]) => [a, b])).toEqual(listed.map(([a, b]) => [a, b]));
  });
  it('hides exactly the controls and those ranges, at every code point', () => {
    for (let cp = 0; cp <= 0x10ffff; cp++) {
      const want =
        cp <= 0x1f || (cp >= 0x7f && cp <= 0x9f) || listed.some(([a, b]) => cp >= a && cp <= b);
      if (hidden(cp) !== want) expect(`U+${cp.toString(16)} ${hidden(cp)}`).toBe(`${want}`);
    }
  });
  it("covers every format, separator and default-ignorable character of this engine's Unicode", () => {
    const invisible = /^[\p{Cc}\p{Cf}\p{Zl}\p{Zp}\p{Default_Ignorable_Code_Point}]$/u;
    for (let cp = 0; cp <= 0x10ffff; cp++) {
      if (surrogate(cp)) continue;
      if (invisible.test(String.fromCodePoint(cp)) && !hidden(cp)) {
        expect(`U+${cp.toString(16)}`).toBe('hidden');
      }
    }
  });
  it('keeps in prose only what the vectors name', () => {
    const kept = vectors.prose.map(hex);
    for (let cp = 0; cp <= 0x10ffff; cp++) {
      if (keptInProse(cp) !== kept.includes(cp)) expect(`U+${cp.toString(16)}`).toBe('kept');
    }
    expect(runs('ok \u2714\ufe0f\n', true).map((r) => r.text)).toEqual(['ok \u2714\ufe0f\n']);
    expect(runs('rm\ufe0f').map((r) => r.text)).toEqual(['rm', '\\u{fe0f}']);
  });
  it('leaves no hidden character in what it shows, whatever the text', () => {
    const pool = [0x41, 0x20, 0x5c, 0x1f600, 0x200d, 0xfe0f, 0xe0041, 0x61c, 0x180e, 0xd800];
    let seed = 7;
    for (let n = 0; n < 2000; n++) {
      let text = '';
      for (let i = 0; i < 12; i++) {
        seed = (seed * 1103515245 + 12345) & 0x7fffffff;
        const cp = seed % 3 === 0 ? (pool[seed % pool.length] as number) : seed % 0x110000;
        text += surrogate(cp) ? String.fromCharCode(cp) : String.fromCodePoint(cp);
      }
      for (const ch of shown(text)) {
        const cp = ch.codePointAt(0) ?? 0;
        if (hidden(cp) || surrogate(cp)) expect(JSON.stringify(text)).toBe('escaped');
      }
    }
  });
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
