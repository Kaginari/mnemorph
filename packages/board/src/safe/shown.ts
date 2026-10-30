/**
 * Text a model or a tool wrote, as a human is shown it: the TypeScript port of the core's
 * `mnemorph_loop::hidden` and `shown`. The two read the same test vectors
 * (`crates/mnemorph-loop/tests/shown-vectors.json`), so they cannot drift apart.
 */

/**
 * Whether the code point `cp` is one a human could not see for what it is: a control character
 * (Unicode Cc), a bidi control, a zero-width or line-separator character.
 */
export function hidden(cp: number): boolean {
  return (
    cp <= 0x1f ||
    (cp >= 0x7f && cp <= 0x9f) ||
    (cp >= 0x200b && cp <= 0x200f) ||
    (cp >= 0x2028 && cp <= 0x202e) ||
    (cp >= 0x2060 && cp <= 0x2069) ||
    cp === 0xfeff
  );
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
 * a newline is kept as written (prose); every other hidden character is escaped as by `shown`.
 */
export function runs(text: string, lines = false): Run[] {
  const out: Run[] = [];
  let plain = '';
  for (const ch of text) {
    const cp = ch.codePointAt(0) ?? 0;
    if (escaped(cp) && !(lines && cp === 0x0a)) {
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
