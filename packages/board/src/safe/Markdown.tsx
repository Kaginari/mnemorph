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

const FENCE = /^ {0,3}(`{3,}|~{3,})\s*([^`\s]*)[^`]*$/;
const HEADING = /^ {0,3}(#{1,6})(?:\s+(.*?))?\s*#*\s*$/;
const RULE = /^ {0,3}([-*_])(?:\s*\1){2,}\s*$/;
const BULLET = /^( {0,3})([-*+])\s+(.*)$/;
const NUMBER = /^( {0,3})(\d{1,9})[.)]\s+(.*)$/;
const QUOTE = /^ {0,3}>\s?(.*)$/;

/** The deepest a quote or a list nests before its text reads flat: a hostile depth costs no
 * stack. */
const DEPTH = 8;

/** The blocks of `text`. */
export function blocks(text: string, depth = 0): Block[] {
  const lines = text.replace(/\r\n?/g, '\n').split('\n');
  const out: Block[] = [];
  let para: string[] = [];
  const flush = () => {
    if (para.length > 0) out.push({ kind: 'p', text: para.join('\n') });
    para = [];
  };
  let i = 0;
  while (i < lines.length) {
    const line = lines[i] ?? '';
    const fence = FENCE.exec(line);
    if (fence?.[1]) {
      flush();
      const mark = fence[1];
      const body: string[] = [];
      i++;
      while (i < lines.length) {
        const l = lines[i] ?? '';
        const close = /^ {0,3}(`{3,}|~{3,})\s*$/.exec(l);
        if (close?.[1] && close[1][0] === mark[0] && close[1].length >= mark.length) break;
        body.push(l);
        i++;
      }
      i++;
      out.push({ kind: 'code', lang: fence[2] ?? '', text: body.join('\n') });
      continue;
    }
    if (line.trim() === '') {
      flush();
      i++;
      continue;
    }
    const h = HEADING.exec(line);
    if (h?.[1]) {
      flush();
      out.push({ kind: 'h', level: h[1].length, text: h[2] ?? '' });
      i++;
      continue;
    }
    if (RULE.test(line)) {
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
        depth < DEPTH ? { kind: 'quote', blocks: blocks(text, depth + 1) } : { kind: 'p', text },
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
        if (m) {
          items.push([m[3] ?? '']);
          i++;
          continue;
        }
        // A continuation: indented, or a lazy line of the item's paragraph.
        const last = items[items.length - 1];
        if (last && l.trim() !== '' && (/^\s{2,}/.test(l) || !(BULLET.test(l) || NUMBER.test(l)))) {
          if (!/^\s/.test(l) && (HEADING.test(l) || FENCE.test(l) || QUOTE.test(l))) break;
          last.push(l.replace(/^\s{1,4}/, ''));
          i++;
          continue;
        }
        if (l.trim() === '' && /^\s{2,}\S/.test(lines[i + 1] ?? '') && last) {
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
          depth < DEPTH ? blocks(it.join('\n'), depth + 1) : [{ kind: 'p', text: it.join('\n') }],
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

/** The closing `]` that matches the `[` at `at`, if any. */
function bracket(s: string, at: number): number {
  let depth = 0;
  for (let i = at; i < s.length && i - at <= WINDOW; i++) {
    const c = s[i];
    if (c === '\\') i++;
    else if (c === '[') depth++;
    else if (c === ']' && --depth === 0) return i;
  }
  return -1;
}

/** A link's target after `](`: up to the matching `)`, an optional title dropped. */
function target(s: string, at: number): [string, number] | undefined {
  let depth = 1;
  for (let i = at; i < s.length && i - at <= WINDOW; i++) {
    const c = s[i];
    if (c === '\\') i++;
    else if (c === '(') depth++;
    else if (c === ')' && --depth === 0) {
      const raw = s.slice(at, i).trim();
      const url = raw.replace(/\s+(?:"[^"]*"|'[^']*')$/, '').replace(/^<(.*)>$/, '$1');
      return [url, i + 1];
    }
  }
  return undefined;
}

const URL_RE = /(?:https?:\/\/|mailto:)[^\s<>]*[^\s<>.,;:!?'")\]]/y;
const AUTO_RE = /<((?:https?:\/\/|mailto:)[^\s<>]+)>/y;
const TICKS = /`+/y;

/** How far a closer is looked for: a hostile run of openers costs no more than this each. */
const WINDOW = 4096;

/** `re` (sticky) matched at `at` in `s`. */
function stuck(re: RegExp, s: string, i: number): RegExpExecArray | null {
  re.lastIndex = i;
  return re.exec(s);
}

/** `needle`'s index in `s` from `from`, within the window. */
function near(s: string, needle: string, from: number): number {
  const end = s.indexOf(needle, from);
  return end >= 0 && end - from <= WINDOW ? end : -1;
}

/** Whether `c` is a word character. */
const word = (c: string | undefined) => c !== undefined && /\w/.test(c);

/** The inlines of `s`. */
export function inlines(s: string, depth = 0): Inline[] {
  const out: Inline[] = [];
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
      if (text.endsWith('  ') || text.endsWith('\\')) {
        text = text.replace(/(?: {2,}|\\)$/, '');
        push({ kind: 'break' });
      } else text += ' ';
      i++;
      continue;
    }
    if (c === '`') {
      const run = stuck(TICKS, s, i)?.[0] ?? '`';
      const end = near(s, run, i + run.length);
      if (end > 0) {
        const code = s.slice(i + run.length, end);
        push({
          kind: 'code',
          text: code.length > 1 && code.trim() ? code.replace(/^ (.*) $/, '$1') : code,
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
      const close = bracket(s, open);
      if (close > 0 && s[close + 1] === '(') {
        const t = target(s, close + 2);
        if (t) {
          const label = s.slice(open + 1, close);
          if (c === '!') push({ kind: 'image', src: t[0], alt: plain(inlines(label, depth + 1)) });
          else push({ kind: 'link', href: t[0], children: inlines(label, depth + 1) });
          i = t[1];
          continue;
        }
      }
    }
    if (c === '<') {
      const auto = stuck(AUTO_RE, s, i);
      if (auto?.[1]) {
        push({ kind: 'link', href: auto[1], children: [{ kind: 'text', text: auto[1] }] });
        i += auto[0].length;
        continue;
      }
    }
    if ((c === 'h' || c === 'm') && !word(text[text.length - 1])) {
      const bare = stuck(URL_RE, s, i);
      if (bare) {
        push({ kind: 'link', href: bare[0], children: [{ kind: 'text', text: bare[0] }] });
        i += bare[0].length;
        continue;
      }
    }
    if (depth < DEPTH && (c === '*' || c === '_' || c === '~')) {
      const run =
        c === '~' ? (s.startsWith('~~', i) ? '~~' : undefined) : s.startsWith(c + c, i) ? c + c : c;
      const after = s[i + (run?.length ?? 0)] ?? '';
      // An opener is followed by a word, and `_` inside a word is a letter (snake_case).
      if (run && after.trim() !== '' && !(c === '_' && word(text[text.length - 1]))) {
        const end = near(s, run, i + run.length + 1);
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
