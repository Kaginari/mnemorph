/**
 * The input line: a pure editor over text and a cursor, with history. Every printable character
 * is text while the input has focus — none is a binding.
 */

export interface Editor {
  text: string;
  /** The cursor, as an index into `text`. */
  cursor: number;
  /** Lines sent before, oldest first. */
  history: string[];
  /** Where ↑ ↓ are in the history; `history.length` is the line being written. */
  at: number;
  /** The line being written, kept while browsing the history. */
  draft: string;
}

export function empty(history: string[] = []): Editor {
  return { text: '', cursor: 0, history, at: history.length, draft: '' };
}

/** Insert `chars` at the cursor (typing or a paste, newlines kept). */
export function insert(e: Editor, chars: string): Editor {
  const clean = chars.replace(/\r\n?/g, '\n');
  return {
    ...e,
    text: e.text.slice(0, e.cursor) + clean + e.text.slice(e.cursor),
    cursor: e.cursor + clean.length,
  };
}

export function backspace(e: Editor): Editor {
  if (e.cursor === 0) return e;
  return {
    ...e,
    text: e.text.slice(0, e.cursor - 1) + e.text.slice(e.cursor),
    cursor: e.cursor - 1,
  };
}

export function del(e: Editor): Editor {
  if (e.cursor >= e.text.length) return e;
  return { ...e, text: e.text.slice(0, e.cursor) + e.text.slice(e.cursor + 1) };
}

export function left(e: Editor): Editor {
  return { ...e, cursor: Math.max(0, e.cursor - 1) };
}

export function right(e: Editor): Editor {
  return { ...e, cursor: Math.min(e.text.length, e.cursor + 1) };
}

/** The start and end of the line the cursor is on. */
function lineBounds(e: Editor): [number, number] {
  const start = e.text.lastIndexOf('\n', e.cursor - 1) + 1;
  const nl = e.text.indexOf('\n', e.cursor);
  return [start, nl === -1 ? e.text.length : nl];
}

export function home(e: Editor): Editor {
  return { ...e, cursor: lineBounds(e)[0] };
}

export function end(e: Editor): Editor {
  return { ...e, cursor: lineBounds(e)[1] };
}

/** Ctrl+U: delete back to the start of the line. */
export function killBack(e: Editor): Editor {
  const [start] = lineBounds(e);
  return { ...e, text: e.text.slice(0, start) + e.text.slice(e.cursor), cursor: start };
}

/** Ctrl+W: delete the word before the cursor. */
export function killWord(e: Editor): Editor {
  const before = e.text.slice(0, e.cursor);
  const start = before.replace(/\S+\s*$/, '').length;
  return { ...e, text: e.text.slice(0, start) + e.text.slice(e.cursor), cursor: start };
}

/** ↑: the line above in the message, else an earlier message. */
export function up(e: Editor): Editor {
  const [start] = lineBounds(e);
  if (start > 0) {
    const prevStart = e.text.lastIndexOf('\n', start - 2) + 1;
    return { ...e, cursor: Math.min(prevStart + (e.cursor - start), start - 1) };
  }
  if (e.at === 0) return e;
  const draft = e.at === e.history.length ? e.text : e.draft;
  const text = e.history[e.at - 1] as string;
  return { ...e, at: e.at - 1, draft, text, cursor: text.length };
}

/** ↓: the line below in the message, else a later message, back to the draft. */
export function down(e: Editor): Editor {
  const [, lineEnd] = lineBounds(e);
  if (lineEnd < e.text.length) {
    const [start] = lineBounds(e);
    const nextEnd = e.text.indexOf('\n', lineEnd + 1);
    const nextLen = (nextEnd === -1 ? e.text.length : nextEnd) - (lineEnd + 1);
    return { ...e, cursor: lineEnd + 1 + Math.min(e.cursor - start, nextLen) };
  }
  if (e.at >= e.history.length) return e;
  const at = e.at + 1;
  const text = at === e.history.length ? e.draft : (e.history[at] as string);
  return { ...e, at, text, cursor: text.length };
}

/** Enter: the message to send, and the editor emptied with it remembered. */
export function submit(e: Editor): [string, Editor] {
  const line = e.text;
  if (!line.trim()) return ['', e];
  const history = e.history.at(-1) === line ? e.history : [...e.history, line];
  return [line, empty(history)];
}
