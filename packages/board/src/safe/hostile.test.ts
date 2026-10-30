/**
 * Hostile length: every parser the board runs on model or tool text — the Markdown subset, the
 * highlighter, the safe-text runs, the router, a call's summary, a diff — on adversarial inputs of
 * about 100 KB, each under 50 ms. A backtracking regex shows here as seconds, not milliseconds.
 */
import { describe, expect, it } from 'vitest';
import { firstLine, summary } from '../data/chat.ts';
import { changeOf } from '../data/diff.ts';
import { parse } from '../router.ts';
import { trimNewlines } from '../ui/Transcript.tsx';
import { tokens } from './Code.tsx';
import { BLOCKS, blocks, fence, heading, parseAll, rule } from './Markdown.tsx';
import { runs } from './shown.ts';

const N = 100_000;
const rep = (s: string, n = N) => s.repeat(Math.ceil(n / s.length)).slice(0, n);

/** The fastest of five runs, in ms: a ReDoS is seconds, a busy machine's noise is not. */
function time(f: () => unknown): number {
  f();
  let best = Number.POSITIVE_INFINITY;
  for (let k = 0; k < 5; k++) {
    const t = performance.now();
    f();
    best = Math.min(best, performance.now() - t);
  }
  return best;
}

const INPUTS: [string, string][] = [
  ['the heading Fable found', `# ${' '.repeat(5000)}#${' '.repeat(5000)}x`],
  ['a heading of spaces and hashes', `# ${rep(' #')}x`],
  ['a heading of hashes', rep('#')],
  ['spaces', rep(' ')],
  ['spaces then a letter', `${rep(' ', N - 1)}x`],
  ['tabs and line separators', rep('\t  ')],
  ['backticks', rep('`')],
  ['backtick pairs', rep('`` ')],
  ['a fence of backticks', `\`\`\`${rep('`')}`],
  ['a fence with a long rest', `\`\`\`${rep(' a')}`],
  ['tildes', rep('~')],
  ['stars', rep('*')],
  ['star words', rep('*a')],
  ['double stars', rep('**a ')],
  ['underscores', rep('_')],
  ['underscore words', rep('_a ')],
  ['open brackets', rep('[')],
  ['open parens', rep('(')],
  ['link starts', rep('[](')],
  ['bracket pairs', rep('[a]')],
  ['link titles', `[a](x${rep(' ')}"t)`],
  ['angle brackets', rep('<')],
  ['autolink starts', rep('<http://')],
  ['bare URL starts', rep(' http://.')],
  ['URL punctuation', `http://${rep('.')}`],
  ['bullets', rep('- ')],
  ['bullet lines', rep('- a\n')],
  ['indented continuations', rep('- a\n   \n  b\n')],
  ['numbers', rep('1. ')],
  ['quotes', rep('> ')],
  ['quote lines', rep('>\n')],
  ['rules', rep('- ')],
  ['rule lines', rep('- - -x\n')],
  ['hard breaks', `a${rep(' ')}\nb`],
  ['backslashes', rep('\\')],
  ['mixed', rep('# *_`[(<h>)]`_* -1. ~~ \\ http:// ')],
  ['newlines', rep('\n')],
  ['slashes', rep('/')],
];

describe('every parser stays linear on hostile input', () => {
  it.each(INPUTS)('the Markdown subset: %s', (_, text) => {
    expect(time(() => parseAll(text))).toBeLessThan(50);
  });

  it.each(INPUTS)('the line readers: %s', (_, text) => {
    expect(time(() => [heading(text), fence(text), rule(text)])).toBeLessThan(50);
  });

  it.each(INPUTS)('the highlighter: %s', (_, text) => {
    expect(time(() => tokens(text.slice(0, 64_000), 'rust'))).toBeLessThan(50);
    expect(time(() => tokens(text.slice(0, 64_000), 'sh'))).toBeLessThan(50);
  });

  it.each(INPUTS)('the safe-text runs: %s', (_, text) => {
    expect(time(() => runs(text))).toBeLessThan(50);
    expect(time(() => runs(text, true))).toBeLessThan(50);
  });

  it.each(INPUTS)('the router, the summary, the trims: %s', (_, text) => {
    expect(
      time(() => [
        parse(`#/${text}`),
        firstLine(text),
        summary('bash', { command: text }),
        trimNewlines(text),
      ]),
    ).toBeLessThan(50);
  });

  it('a diff of two long strings', () => {
    const a = Array.from({ length: 1000 }, (_, i) => `line ${i}`).join('\n');
    const b = Array.from({ length: 1000 }, (_, i) => `line ${i * 7}`).join('\n');
    expect(time(() => changeOf('edit', { old_string: a, new_string: b }))).toBeLessThan(50);
    expect(time(() => changeOf('patch', { patch: rep('+a\n-b\n@@ -1 +1 @@\n') }))).toBeLessThan(50);
  });

  it('a hostile count of blocks is cut, the rest one paragraph', () => {
    const bs = blocks('- a\n'.repeat(25_000));
    const items = bs[0]?.kind === 'list' ? bs[0].items.length : 0;
    expect(items).toBeLessThanOrEqual(BLOCKS);
    expect(bs.at(-1)?.kind).toBe('p');
  });

  it('the heading reads as CommonMark says', () => {
    expect(heading('# Title ##')).toEqual({ level: 1, text: 'Title' });
    expect(heading('### a # b')).toEqual({ level: 3, text: 'a # b' });
    expect(heading('## #')).toEqual({ level: 2, text: '' });
    expect(heading('#hashtag')).toBeUndefined();
    expect(heading('####### seven')).toBeUndefined();
    expect(fence('```py extra')).toEqual({ mark: '```', lang: 'py' });
    expect(fence('``` a`b')).toBeUndefined();
    expect(fence('~~~ a`b')).toEqual({ mark: '~~~', lang: 'a`b' });
    expect(rule(' * * *')).toBe(true);
    expect(rule('- - x')).toBe(false);
  });
});
