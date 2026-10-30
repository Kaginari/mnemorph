/**
 * The body's Markdown, rendered inert: a small subset parsed here — paragraphs, headings, lists,
 * quotes, rules, fenced code, code spans, strong, emphasis, strikethrough, links and images —
 * into React elements whose every word is a `<Safe>` text node. Raw HTML is never parsed: it
 * reads as the text it is. Links go through `SafeLink` (http, https, mailto only); images through
 * `SafeImage` (never loaded). No dependency, nothing from a string becomes markup, nothing runs.
 */
import type { ReactNode } from 'react';
import { Code } from './Code.tsx';
import { Safe, SafeImage, SafeLink } from './Safe.tsx';

export type Block =
  | { kind: 'p'; text: string }
  | { kind: 'h'; level: number; text: string }
  | { kind: 'code'; lang: string; text: string }
  | { kind: 'quote'; blocks: Block[] }
  | { kind: 'list'; ordered: boolean; start: number; items: Block[][] }
  | { kind: 'rule' };

// Each line is read by a linear scan or a regex with no overlapping quantifiers: model text is
// hostile, and a backtracking pattern on a long line would freeze every tab that shows it. The
// `s` flag lets `.` match a line separator too, so `(.*)$` never fails and backtracks.
const BULLET = /^( {0,3})([-*+])[ \t]+(.*)$/s;
const NUMBER = /^( {0,3})(\d{1,9})[.)][ \t]+(.*)$/s;
const QUOTE = /^ {0,3}>[ \t]?(.*)$/s;

/** A fence's opening: its run (three or more of one mark) and its language; a backtick fence
 * whose rest holds a backtick is no fence. */
export function fence(line: string): { mark: string; lang: string } | undefined {
  const m = /^ {0,3}(`{3,}|~{3,})(.*)$/s.exec(line);
  const run = m?.[1];
  if (!run) return undefined;
  const rest = m[2] ?? '';
  if (run[0] === '`' && rest.includes('`')) return undefined;
  return { mark: run, lang: rest.trim().split(/[ \t]/)[0] ?? '' };
}

/** Whether `line` closes a fence opened by `mark`: the same mark, at least as long, then blanks. */
function closes(line: string, mark: string): boolean {
  const m = /^ {0,3}(`{3,}|~{3,})(.*)$/s.exec(line);
  const run = m?.[1];
  return !!run && run[0] === mark[0] && run.length >= mark.length && (m[2] ?? '').trim() === '';
}

/** A heading: its level and its text, the closing hashes dropped. */
export function heading(line: string): { level: number; text: string } | undefined {
  const m = /^ {0,3}(#{1,6})(?:[ \t]+|$)(.*)$/s.exec(line);
  const hashes = m?.[1];
  if (!hashes) return undefined;
  let text = (m[2] ?? '').trimEnd();
  // The closing run of hashes, after a blank, is no part of the text.
  let end = text.length;
  while (end > 0 && text[end - 1] === '#') end--;
  if (end === 0) text = '';
  else if (end < text.length && (text[end - 1] === ' ' || text[end - 1] === '\t'))
    text = text.slice(0, end);
  return { level: hashes.length, text: text.trimEnd() };
}

/** Whether `line` is a thematic break: three or more of one of `-`, `*`, `_`, blanks between. */
export function rule(line: string): boolean {
  let indent = 0;
  while (line[indent] === ' ') indent++;
  if (indent > 3) return false;
  const mark = line[indent];
  if (mark !== '-' && mark !== '*' && mark !== '_') return false;
  let n = 0;
  for (let i = indent; i < line.length; i++) {
    const c = line[i];
    if (c === mark) n++;
    else if (c !== ' ' && c !== '\t') return false;
  }
  return n >= 3;
}

/** Whether `line` opens no block of its own: it starts with none of a block's marks. */
function plainLine(line: string): boolean {
  const c = line.trimStart()[0];
  return c !== undefined && !'#`~-*_+>0123456789'.includes(c);
}

/** The number of leading blanks (spaces or tabs) of `line`. */
function blanks(line: string): number {
  let n = 0;
  while (n < line.length && (line[n] === ' ' || line[n] === '\t')) n++;
  return n;
}

/** The deepest a quote or a list nests before its text reads flat: a hostile depth costs no
 * stack. */
const DEPTH = 8;

/** The most blocks one text parses into, nested ones counted: past it, the rest is one paragraph.
 * A hostile count of tiny blocks costs bounded work, to parse and to draw. */
export const BLOCKS = 2000;

/** The blocks of `text`; `budget` is shared by the nested parses. */
export function blocks(text: string, depth = 0, budget = { left: BLOCKS }): Block[] {
  // One plain line is one paragraph: a list of many short items parses each at no cost.
  if (!text.includes('\n') && !text.includes('\r') && plainLine(text)) {
    return text.trim() === '' ? [] : [{ kind: 'p', text }];
  }
  const lines = text.replaceAll('\r\n', '\n').replaceAll('\r', '\n').split('\n');
  const out: Block[] = [];
  let para: string[] = [];
  const flush = () => {
    if (para.length > 0) out.push({ kind: 'p', text: para.join('\n') });
    para = [];
  };
  let i = 0;
  while (i < lines.length) {
    if (budget.left <= 0) {
      flush();
      const rest = lines.slice(i).join('\n');
      if (rest.trim() !== '') out.push({ kind: 'p', text: rest });
      break;
    }
    budget.left--;
    const line = lines[i] ?? '';
    const f = fence(line);
    if (f) {
      flush();
      const body: string[] = [];
      i++;
      while (i < lines.length) {
        const l = lines[i] ?? '';
        if (closes(l, f.mark)) break;
        body.push(l);
        i++;
      }
      i++;
      out.push({ kind: 'code', lang: f.lang, text: body.join('\n') });
      continue;
    }
    if (line.trim() === '') {
      flush();
      i++;
      continue;
    }
    const h = heading(line);
    if (h) {
      flush();
      out.push({ kind: 'h', level: h.level, text: h.text });
      i++;
      continue;
    }
    if (rule(line)) {
      flush();
      out.push({ kind: 'rule' });
      i++;
      continue;
    }
    if (QUOTE.test(line)) {
      flush();
      const inner: string[] = [];
      while (i < lines.length) {
        const q = QUOTE.exec(lines[i] ?? '');
        if (!q) break;
        inner.push(q[1] ?? '');
        i++;
      }
      const text = inner.join('\n');
      out.push(
        depth < DEPTH
          ? { kind: 'quote', blocks: blocks(text, depth + 1, budget) }
          : { kind: 'p', text },
      );
      continue;
    }
    const bullet = BULLET.exec(line);
    const number = NUMBER.exec(line);
    if (bullet || number) {
      flush();
      const ordered = !bullet;
      const start = number ? Number(number[2]) : 1;
      const items: string[][] = [];
      while (i < lines.length) {
        const l = lines[i] ?? '';
        const m = ordered ? NUMBER.exec(l) : BULLET.exec(l);
        if (m && items.length > 0 && budget.left-- <= 0) break;
        if (m) {
          items.push([m[3] ?? '']);
          i++;
          continue;
        }
        // A continuation: indented, or a lazy line of the item's paragraph.
        const last = items[items.length - 1];
        if (last && l.trim() !== '' && (blanks(l) >= 2 || !(BULLET.test(l) || NUMBER.test(l)))) {
          if (blanks(l) === 0 && (heading(l) || fence(l) || QUOTE.test(l))) break;
          last.push(l.slice(Math.min(4, blanks(l))));
          i++;
          continue;
        }
        const next = lines[i + 1] ?? '';
        if (l.trim() === '' && blanks(next) >= 2 && next.trim() !== '' && last) {
          last.push('');
          i++;
          continue;
        }
        break;
      }
      out.push({
        kind: 'list',
        ordered,
        start,
        items: items.map((it) =>
          depth < DEPTH
            ? blocks(it.join('\n'), depth + 1, budget)
            : [{ kind: 'p', text: it.join('\n') }],
        ),
      });
      continue;
    }
    para.push(line);
    i++;
  }
  flush();
  return out;
}

export type Inline =
  | { kind: 'text'; text: string }
  | { kind: 'code'; text: string }
  | { kind: 'strong' | 'em' | 'del'; children: Inline[] }
  | { kind: 'link'; href: string; children: Inline[] }
  | { kind: 'image'; src: string; alt: string }
  | { kind: 'break' };

/** The text of a run of inlines, as plain words. */
function plain(xs: Inline[]): string {
  return xs
    .map((x) => {
      switch (x.kind) {
        case 'text':
        case 'code':
          return x.text;
        case 'image':
          return x.alt;
        case 'break':
          return ' ';
        default:
          return plain(x.children);
      }
    })
    .join('');
}

/** Each bracket's match, by one pass with a stack: `[`→`]` and `(`→`)`, a backslash escaping
 * the next character. Linear, so a run of openers costs nothing per opener. */
function matches(s: string): Map<number, number> {
  const out = new Map<number, number>();
  const squares: number[] = [];
  const rounds: number[] = [];
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === '\\') i++;
    else if (c === '[') squares.push(i);
    else if (c === '(') rounds.push(i);
    else if (c === ']') {
      const o = squares.pop();
      if (o !== undefined) out.set(o, i);
    } else if (c === ')') {
      const o = rounds.pop();
      if (o !== undefined) out.set(o, i);
    }
  }
  return out;
}

/** A link's target, the text between its parentheses: an optional title dropped, `<…>` unwrapped. */
function linkTarget(raw: string): string {
  let t = raw.trim();
  const q = t[t.length - 1];
  if (t.length > 1 && (q === '"' || q === "'")) {
    const open = t.lastIndexOf(q, t.length - 2);
    if (open > 0 && (t[open - 1] === ' ' || t[open - 1] === '\t' || t[open - 1] === '\n')) {
      t = t.slice(0, open).trimEnd();
    }
  }
  if (t.length >= 2 && t[0] === '<' && t[t.length - 1] === '>') t = t.slice(1, -1);
  return t;
}

/** Whether `c` ends no URL: blank, `<` or `>`. */
const stops = (c: string | undefined) =>
  c === undefined || c === '<' || c === '>' || c.trim() === '';

/** The URL starting at `i` (http, https or mailto), its trailing punctuation left out; empty when
 * none. A linear scan. */
function urlAt(s: string, i: number): string {
  const scheme = s.startsWith('https://', i)
    ? 8
    : s.startsWith('http://', i)
      ? 7
      : s.startsWith('mailto:', i)
        ? 7
        : 0;
  if (scheme === 0) return '';
  let end = i + scheme;
  while (end < s.length && !stops(s[end])) end++;
  while (end > i + scheme && '.,;:!?\'")]'.includes(s[end - 1] ?? '')) end--;
  return end > i + scheme ? s.slice(i, end) : '';
}

/** How far a closer is looked for. */
const WINDOW = 4096;

/**
 * `needle`'s first index at or after `from` within the window, remembered per needle: the next
 * search from further on starts where the last one found, so a run of openers scans the text once.
 */
function finder(s: string): (needle: string, from: number) => number {
  const seen = new Map<string, { from: number; at: number }>();
  return (needle, from) => {
    const last = seen.get(needle);
    let at: number;
    if (last && last.from <= from && (last.at === -1 || last.at >= from)) at = last.at;
    else {
      at = s.indexOf(needle, from);
      seen.set(needle, { from, at });
    }
    return at >= 0 && at - from <= WINDOW ? at : -1;
  };
}

/** Whether `c` is a word character. */
const word = (c: string | undefined) => c !== undefined && /\w/.test(c);

/** The inlines of `s`. */
export function inlines(s: string, depth = 0): Inline[] {
  const out: Inline[] = [];
  const near = finder(s);
  let pairs: Map<number, number> | undefined;
  const pair = (at: number) => {
    pairs ??= matches(s);
    return pairs.get(at) ?? -1;
  };
  let text = '';
  const push = (x: Inline) => {
    if (text) out.push({ kind: 'text', text });
    text = '';
    out.push(x);
  };
  let i = 0;
  while (i < s.length) {
    const c = s[i] ?? '';
    if (c === '\\' && i + 1 < s.length && /[\\`*_{}[\]()#+\-.!~<>|]/.test(s[i + 1] ?? '')) {
      text += s[i + 1];
      i += 2;
      continue;
    }
    if (c === '\n') {
      // Read from the source: indexing the text being built would flatten it each time.
      const back = s[i - 1] === '\\';
      if (back || (s[i - 1] === ' ' && s[i - 2] === ' ')) {
        text = back ? text.slice(0, -1) : text.trimEnd();
        push({ kind: 'break' });
      } else text += ' ';
      i++;
      continue;
    }
    if (c === '`') {
      let n = i;
      while (s[n] === '`') n++;
      const run = s.slice(i, n);
      const end = near(run, i + run.length);
      if (end > 0) {
        const code = s.slice(i + run.length, end);
        push({
          kind: 'code',
          text:
            code.length > 2 && code.trim() && code[0] === ' ' && code[code.length - 1] === ' '
              ? code.slice(1, -1)
              : code,
        });
        i = end + run.length;
        continue;
      }
      text += run;
      i += run.length;
      continue;
    }
    if (depth < DEPTH && (c === '[' || (c === '!' && s[i + 1] === '['))) {
      const open = c === '!' ? i + 1 : i;
      const close = pair(open);
      const end = close > 0 && s[close + 1] === '(' ? pair(close + 1) : -1;
      if (end > 0) {
        const href = linkTarget(s.slice(close + 2, end));
        const label = s.slice(open + 1, close);
        if (c === '!') push({ kind: 'image', src: href, alt: plain(inlines(label, depth + 1)) });
        else push({ kind: 'link', href, children: inlines(label, depth + 1) });
        i = end + 1;
        continue;
      }
    }
    if (c === '<') {
      let end = i + 1;
      while (end < s.length && !stops(s[end])) end++;
      const href = s.slice(i + 1, end);
      if (s[end] === '>' && urlAt(href, 0) !== '' && href.length > 7) {
        push({ kind: 'link', href, children: [{ kind: 'text', text: href }] });
        i = end + 1;
        continue;
      }
    }
    if ((c === 'h' || c === 'm') && !word(s[i - 1])) {
      const bare = urlAt(s, i);
      if (bare) {
        push({ kind: 'link', href: bare, children: [{ kind: 'text', text: bare }] });
        i += bare.length;
        continue;
      }
    }
    if (depth < DEPTH && (c === '*' || c === '_' || c === '~')) {
      const run =
        c === '~' ? (s.startsWith('~~', i) ? '~~' : undefined) : s.startsWith(c + c, i) ? c + c : c;
      const after = s[i + (run?.length ?? 0)] ?? '';
      // An opener is followed by a word, and `_` inside a word is a letter (snake_case).
      if (run && after.trim() !== '' && !(c === '_' && word(s[i - 1]))) {
        const end = near(run, i + run.length + 1);
        const before = s[end - 1] ?? '';
        if (
          end > 0 &&
          before.trim() !== '' &&
          !(c === '_' && /\w/.test(s[end + run.length] ?? ''))
        ) {
          const kind = c === '~' ? 'del' : run.length === 2 ? 'strong' : 'em';
          push({ kind, children: inlines(s.slice(i + run.length, end), depth + 1) });
          i = end + run.length;
          continue;
        }
      }
    }
    text += c;
    i++;
  }
  if (text) out.push({ kind: 'text', text });
  return out;
}

/** Every block of `text` and the inlines of each, as the page renders them, without React: the
 * whole parse, for a test to time. */
export function parseAll(text: string): number {
  let n = 0;
  const walk = (bs: Block[]) => {
    for (const b of bs) {
      n++;
      if (b.kind === 'p' || b.kind === 'h') n += inlines(b.text).length;
      else if (b.kind === 'quote') walk(b.blocks);
      else if (b.kind === 'list') for (const it of b.items) walk(it);
    }
  };
  walk(blocks(text));
  return n;
}

function Inlines({ xs }: { xs: Inline[] }): ReactNode {
  return xs.map((x, i) => {
    switch (x.kind) {
      case 'text':
        // biome-ignore lint/suspicious/noArrayIndexKey: inlines never reorder
        return <Safe key={i} text={x.text} />;
      case 'code':
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: inlines never reorder
          <code key={i}>
            <Safe text={x.text} />
          </code>
        );
      case 'strong':
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: inlines never reorder
          <strong key={i}>
            <Inlines xs={x.children} />
          </strong>
        );
      case 'em':
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: inlines never reorder
          <em key={i}>
            <Inlines xs={x.children} />
          </em>
        );
      case 'del':
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: inlines never reorder
          <del key={i}>
            <Inlines xs={x.children} />
          </del>
        );
      case 'link':
        // biome-ignore lint/suspicious/noArrayIndexKey: inlines never reorder
        return <SafeLink key={i} href={x.href} text={plain(x.children)} />;
      case 'image':
        // biome-ignore lint/suspicious/noArrayIndexKey: inlines never reorder
        return <SafeImage key={i} src={x.src} alt={x.alt} />;
      case 'break':
        // biome-ignore lint/suspicious/noArrayIndexKey: inlines never reorder
        return <br key={i} />;
      default:
        return null;
    }
  });
}

function Blocks({ bs }: { bs: Block[] }): ReactNode {
  return bs.map((b, i) => {
    switch (b.kind) {
      case 'p':
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: blocks never reorder
          <p key={i}>
            <Inlines xs={inlines(b.text)} />
          </p>
        );
      case 'h': {
        const H = `h${Math.min(6, b.level + 2)}` as 'h3' | 'h4' | 'h5' | 'h6';
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: blocks never reorder
          <H key={i} className="md-h">
            <Inlines xs={inlines(b.text)} />
          </H>
        );
      }
      case 'code':
        // biome-ignore lint/suspicious/noArrayIndexKey: blocks never reorder
        return <Code key={i} lang={b.lang} text={b.text} />;
      case 'quote':
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: blocks never reorder
          <blockquote key={i}>
            <Blocks bs={b.blocks} />
          </blockquote>
        );
      case 'list': {
        const items = b.items.map((it, j) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: items never reorder
          <li key={j}>
            <Blocks bs={it} />
          </li>
        ));
        return b.ordered ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: blocks never reorder
          <ol key={i} start={b.start}>
            {items}
          </ol>
        ) : (
          // biome-ignore lint/suspicious/noArrayIndexKey: blocks never reorder
          <ul key={i}>{items}</ul>
        );
      }
      case 'rule':
        // biome-ignore lint/suspicious/noArrayIndexKey: blocks never reorder
        return <hr key={i} />;
      default:
        return null;
    }
  });
}

/** Past this many characters, text is shown as it is, not parsed. */
export const MARKDOWN_CHARS = 100_000;

/** Markdown from a model or a tool, rendered inert. */
export function Markdown({ text }: { text: string }) {
  if (text.length > MARKDOWN_CHARS) {
    return (
      <div className="md">
        <p>
          <Safe text={text} lines />
        </p>
      </div>
    );
  }
  return (
    <div className="md">
      <Blocks bs={blocks(text)} />
    </div>
  );
}
