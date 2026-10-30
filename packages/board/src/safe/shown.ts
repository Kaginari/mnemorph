/**
 * Text a model or a tool wrote, as a human is shown it: the TypeScript port of the core's
 * `mnemorph_loop::hidden` and `shown`. The two read the same test vectors
 * (`crates/mnemorph-loop/tests/shown-vectors.json`), so they cannot drift apart.
 */

/**
 * The code points `hidden` escapes beyond Unicode's controls (Cc), as sorted, inclusive ranges:
 * every format character (Cf), the line and paragraph separators (Zl, Zp) and every
 * default-ignorable code point. The shared vectors list the same ranges, each with its reason,
 * and the tests check every code point against them.
 */
export const HIDDEN: readonly (readonly [number, number])[] = [
  [0xad, 0xad],
  [0x34f, 0x34f],
  [0x600, 0x605],
  [0x61c, 0x61c],
  [0x6dd, 0x6dd],
  [0x70f, 0x70f],
  [0x890, 0x891],
  [0x8e2, 0x8e2],
  [0x115f, 0x1160],
  [0x17b4, 0x17b5],
  [0x180b, 0x180f],
  [0x200b, 0x200f],
  [0x2028, 0x202e],
  [0x2060, 0x206f],
  [0x3164, 0x3164],
  [0xfe00, 0xfe0f],
  [0xfeff, 0xfeff],
  [0xffa0, 0xffa0],
  [0xfff0, 0xfffb],
  [0x110bd, 0x110bd],
  [0x110cd, 0x110cd],
  [0x13430, 0x1343f],
  [0x1bca0, 0x1bca3],
  [0x1d173, 0x1d17a],
  [0xe0000, 0xe0fff],
];

/**
 * Whether the code point `cp` is one a human could not see for what it is: a control character
 * (Unicode Cc), or one of `HIDDEN` — a format character, a bidi control, a line or paragraph
 * separator, a default-ignorable code point.
 */
export function hidden(cp: number): boolean {
  if (cp <= 0x1f || (cp >= 0x7f && cp <= 0x9f)) return true;
  let lo = 0;
  let hi = HIDDEN.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const [from, to] = HIDDEN[mid] as readonly [number, number];
    if (cp < from) hi = mid - 1;
    else if (cp > to) lo = mid + 1;
    else return true;
  }
  return false;
}

/**
 * Whether prose keeps `cp` as written though it is hidden: U+FE0F alone, which only asks for the
 * emoji form of the character before it. A name, a path or an act escapes it.
 */
export function keptInProse(cp: number): boolean {
  return cp === 0xfe0f;
}

/** A lone surrogate: a JavaScript string can hold one, a Rust string cannot. Escaped too. */
function lone(cp: number): boolean {
  return cp >= 0xd800 && cp <= 0xdfff;
}

/** `\u{…}`, as Rust's `char::escape_unicode` writes it. */
export function escapeOf(cp: number): string {
  return `\\u{${cp.toString(16)}}`;
}

/** Whether `cp` is escaped by `shown`: hidden, a backslash, or a lone surrogate. */
export function escaped(cp: number): boolean {
  return hidden(cp) || cp === 0x5c || lone(cp);
}

/**
 * `text` with every hidden character escaped as `\u{…}`, and a backslash doubled the same way so
 * an escape can never be mistaken for one written in the text.
 */
export function shown(text: string): string {
  let out = '';
  for (const ch of text) {
    const cp = ch.codePointAt(0) ?? 0;
    out += escaped(cp) ? escapeOf(cp) : ch;
  }
  return out;
}

/** One run of shown text: either as written, or one escaped character. */
export interface Run {
  text: string;
  escaped: boolean;
}

/**
 * `text` as runs, so the page can draw an escape apart from the text around it. With `lines`,
 * a newline and what `keptInProse` names are kept as written (prose); every other hidden
 * character is escaped as by `shown`.
 */
export function runs(text: string, lines = false): Run[] {
  const out: Run[] = [];
  let plain = '';
  for (const ch of text) {
    const cp = ch.codePointAt(0) ?? 0;
    if (escaped(cp) && !(lines && (cp === 0x0a || keptInProse(cp)))) {
      if (plain) out.push({ text: plain, escaped: false });
      plain = '';
      out.push({ text: escapeOf(cp), escaped: true });
    } else {
      plain += ch;
    }
  }
  if (plain) out.push({ text: plain, escaped: false });
  return out;
}
