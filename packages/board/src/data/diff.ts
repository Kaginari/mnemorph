/**
 * What a file tool changes, as a diff with its line numbers — as the TUI draws it
 * (crates/mnemorph-tui/src/diff.rs): `edit` and `multiedit` from their old and new strings,
 * `patch` from its unified diff, `write` as the new file's head.
 */

export type DiffLine =
  | { kind: 'same'; text: string; n: number }
  | { kind: 'add'; text: string; n: number }
  | { kind: 'del'; text: string }
  | { kind: 'hunk'; text: string };

export type Change =
  | { kind: 'diff'; lines: DiffLine[]; added: number; removed: number }
  | { kind: 'file'; path: string; lines: number; head: string[] };

/** The context kept around each change. */
const CONTEXT = 2;
/** The lines of a written file shown before it folds. */
export const HEAD = 4;
/** Past this many lines on a side, an edit is shown removed and added whole: the diff would cost
 * more than it shows. */
const DIFF_CAP = 2000;

const s = (v: unknown, k: string): string | undefined => {
  const x = (v as Record<string, unknown> | null)?.[k];
  return typeof x === 'string' ? x : undefined;
};

const lines = (t: string): string[] => {
  const l = t.split('\n');
  if (l.length > 0 && l[l.length - 1] === '') l.pop();
  return l;
};

/** The change a call makes, when it is a file tool's. */
export function changeOf(name: string, input: unknown): Change | undefined {
  switch (name) {
    case 'edit': {
      const o = s(input, 'old_string');
      const n = s(input, 'new_string');
      return o === undefined || n === undefined ? undefined : edits([[o, n]]);
    }
    case 'multiedit': {
      const list = (input as { edits?: unknown } | null)?.edits;
      if (!Array.isArray(list)) return undefined;
      const pairs: [string, string][] = [];
      for (const e of list) {
        const o = s(e, 'old_string');
        const n = s(e, 'new_string');
        if (o !== undefined && n !== undefined) pairs.push([o, n]);
      }
      return edits(pairs);
    }
    case 'patch': {
      const p = s(input, 'patch');
      return p === undefined ? undefined : patch(p);
    }
    case 'write': {
      const c = s(input, 'content');
      if (c === undefined) return undefined;
      const all = lines(c);
      return {
        kind: 'file',
        path: s(input, 'path') ?? '',
        lines: all.length,
        head: all.slice(0, HEAD),
      };
    }
    default:
      return undefined;
  }
}

type Op = ['=' | '+' | '-', string];

/** The line diff of `a` to `b`: the longest common subsequence, ties to removals first. */
export function diffLines(a: string[], b: string[]): Op[] {
  if (a.length > DIFF_CAP || b.length > DIFF_CAP) {
    return [...a.map((t): Op => ['-', t]), ...b.map((t): Op => ['+', t])];
  }
  // Common head and tail first: an edit usually changes little in a long string.
  let head = 0;
  while (head < a.length && head < b.length && a[head] === b[head]) head++;
  let tail = 0;
  while (
    tail < a.length - head &&
    tail < b.length - head &&
    a[a.length - 1 - tail] === b[b.length - 1 - tail]
  )
    tail++;
  const x = a.slice(head, a.length - tail);
  const y = b.slice(head, b.length - tail);
  const w = y.length + 1;
  const lcs = new Uint32Array((x.length + 1) * w);
  for (let i = x.length - 1; i >= 0; i--) {
    for (let j = y.length - 1; j >= 0; j--) {
      lcs[i * w + j] =
        x[i] === y[j]
          ? (lcs[(i + 1) * w + j + 1] ?? 0) + 1
          : Math.max(lcs[(i + 1) * w + j] ?? 0, lcs[i * w + j + 1] ?? 0);
    }
  }
  const mid: Op[] = [];
  let i = 0;
  let j = 0;
  while (i < x.length && j < y.length) {
    if (x[i] === y[j]) {
      mid.push(['=', x[i] ?? '']);
      i++;
      j++;
    } else if ((lcs[(i + 1) * w + j] ?? 0) >= (lcs[i * w + j + 1] ?? 0)) {
      mid.push(['-', x[i] ?? '']);
      i++;
    } else {
      mid.push(['+', y[j] ?? '']);
      j++;
    }
  }
  while (i < x.length) mid.push(['-', x[i++] ?? '']);
  while (j < y.length) mid.push(['+', y[j++] ?? '']);
  return [
    ...a.slice(0, head).map((t): Op => ['=', t]),
    ...mid,
    ...a.slice(a.length - tail).map((t): Op => ['=', t]),
  ];
}

/** Each old→new pair diffed by line, `CONTEXT` lines kept around each change, a gap between
 * pairs and between distant hunks. A pair's lines are numbered from 1: the string's own, as the
 * file's line is not known. */
function edits(pairs: [string, string][]): Change {
  const out: DiffLine[] = [];
  let added = 0;
  let removed = 0;
  for (const [o, n] of pairs) {
    const ops = diffLines(lines(o), lines(n));
    const keep = ops.map(() => false);
    ops.forEach(([k], at) => {
      if (k === '=') return;
      for (let d = Math.max(0, at - CONTEXT); d <= Math.min(ops.length - 1, at + CONTEXT); d++)
        keep[d] = true;
    });
    let num = 0;
    let gap = out.length > 0;
    ops.forEach(([k, text], at) => {
      if (k !== '-') num++;
      if (!keep[at]) {
        gap = true;
        return;
      }
      if (gap && out.length > 0) out.push({ kind: 'hunk', text: '⋯' });
      gap = false;
      if (k === '=') out.push({ kind: 'same', text, n: num });
      else if (k === '+') {
        added++;
        out.push({ kind: 'add', text, n: num });
      } else {
        removed++;
        out.push({ kind: 'del', text });
      }
    });
  }
  return { kind: 'diff', lines: out, added, removed };
}

/** The first new-side line number of a hunk header, `@@ -a,b +c,d @@`. */
function hunkStart(h: string): number | undefined {
  const m = /\+(\d+)/.exec(h);
  return m?.[1] ? Number(m[1]) : undefined;
}

/** A unified diff read line by line: its file headers dropped, its hunk headers kept, the new
 * side's lines numbered from each hunk's start. */
function patch(text: string): Change {
  const out: DiffLine[] = [];
  let added = 0;
  let removed = 0;
  let at = 1;
  for (const l of lines(text)) {
    if (l.startsWith('+++') || l.startsWith('---') || l.startsWith('diff ')) continue;
    if (l.startsWith('@@')) {
      at = hunkStart(l) ?? at;
      out.push({ kind: 'hunk', text: l });
    } else if (l.startsWith('+')) {
      added++;
      out.push({ kind: 'add', text: l.slice(1), n: at++ });
    } else if (l.startsWith('-')) {
      removed++;
      out.push({ kind: 'del', text: l.slice(1) });
    } else {
      out.push({ kind: 'same', text: l.startsWith(' ') ? l.slice(1) : l, n: at++ });
    }
  }
  return { kind: 'diff', lines: out, added, removed };
}

/** The `+3 −1` summary, or the written file's size. */
export function stat(c: Change): string {
  return c.kind === 'diff' ? `+${c.added} −${c.removed}` : `${c.lines} lines`;
}
