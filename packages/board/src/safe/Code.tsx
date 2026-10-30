/**
 * A fenced code block: its language named, its lines numbered, lightly highlighted — strings,
 * comments, numbers and the common keywords — by a small tokenizer here. Every token is a `<Safe>`
 * text node in a span whose class names its kind: no markup from a string, no dependency.
 */
import { Safe } from './Safe.tsx';

export type Kind = 'plain' | 'str' | 'com' | 'num' | 'key';

const KEYWORDS = new Set(
  (
    'as async await break case catch class const continue def default defer del do elif else ' +
    'enum except export extends false final finally fn for from func function go if impl import ' +
    'in interface is let loop match mod mut new nil none null pass pub raise return self static ' +
    'struct super switch this throw trait true try type typeof undefined use var void where ' +
    'while with yield True False None'
  ).split(' '),
);

/** Languages whose comments start with `#`. */
const HASH = new Set([
  'py',
  'python',
  'sh',
  'bash',
  'shell',
  'zsh',
  'toml',
  'yaml',
  'yml',
  'ruby',
  'rb',
  'r',
  'make',
  'dockerfile',
]);

/** The longest a block is tokenized; past it, it is shown plain. */
const HIGHLIGHT_CHARS = 64_000;

/** The tokens of `text`: each run of one kind, in order, joined they are `text` again. */
export function tokens(text: string, lang: string): [Kind, string][] {
  if (text.length > HIGHLIGHT_CHARS) return [['plain', text]];
  const hash = HASH.has(lang.toLowerCase());
  const out: [Kind, string][] = [];
  const push = (k: Kind, s: string) => {
    const last = out[out.length - 1];
    if (last && last[0] === k) last[1] += s;
    else out.push([k, s]);
  };
  let i = 0;
  while (i < text.length) {
    const c = text[i] ?? '';
    const two = text.slice(i, i + 2);
    if ((hash && c === '#') || (!hash && two === '//') || (two === '--' && lang === 'sql')) {
      const end = text.indexOf('\n', i);
      const stop = end < 0 ? text.length : end;
      push('com', text.slice(i, stop));
      i = stop;
      continue;
    }
    if (!hash && two === '/*') {
      const end = text.indexOf('*/', i + 2);
      const stop = end < 0 ? text.length : end + 2;
      push('com', text.slice(i, stop));
      i = stop;
      continue;
    }
    if (c === '"' || c === "'" || c === '`') {
      let j = i + 1;
      while (j < text.length && text[j] !== c && text[j] !== '\n') j += text[j] === '\\' ? 2 : 1;
      const stop = Math.min(text.length, j + 1);
      push('str', text.slice(i, stop));
      i = stop;
      continue;
    }
    if (/[0-9]/.test(c) && !/[\w$]/.test(text[i - 1] ?? '')) {
      let j = i;
      while (j < text.length && /[\w.]/.test(text[j] ?? '')) j++;
      push('num', text.slice(i, j));
      i = j;
      continue;
    }
    if (/[A-Za-z_$]/.test(c)) {
      let j = i;
      while (j < text.length && /[\w$]/.test(text[j] ?? '')) j++;
      const w = text.slice(i, j);
      push(KEYWORDS.has(w) ? 'key' : 'plain', w);
      i = j;
      continue;
    }
    push('plain', c);
    i++;
  }
  return out;
}

/** The tokens split into lines, a token across a line break split with it. */
function lines(ts: [Kind, string][]): [Kind, string][][] {
  const out: [Kind, string][][] = [[]];
  for (const [k, s] of ts) {
    const parts = s.split('\n');
    parts.forEach((p, n) => {
      if (n > 0) out.push([]);
      if (p) out[out.length - 1]?.push([k, p]);
    });
  }
  return out;
}

export function Code({ lang, text }: { lang: string; text: string }) {
  const rows = lines(tokens(text, lang));
  return (
    <figure className="code">
      {lang && (
        <figcaption className="code-lang">
          <Safe text={lang} />
        </figcaption>
      )}
      <pre className="code-body">
        {rows.map((row, n) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: lines never reorder
          <span key={n} className="code-line">
            <span className="code-n" aria-hidden="true">
              {n + 1}
            </span>
            <code className="code-text">
              {row.map(([k, s], j) =>
                k === 'plain' ? (
                  // biome-ignore lint/suspicious/noArrayIndexKey: tokens never reorder
                  <Safe key={j} text={s} />
                ) : (
                  // biome-ignore lint/suspicious/noArrayIndexKey: tokens never reorder
                  <span key={j} className={`tok-${k}`}>
                    <Safe text={s} />
                  </span>
                ),
              )}
            </code>
          </span>
        ))}
      </pre>
    </figure>
  );
}
