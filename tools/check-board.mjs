// The board's own lint, beside Biome: colours come from @mnemorph/theme's tokens only (no raw
// hex, rgb(), hsl() or other colour literal in packages/board), and no HTML is ever written as a
// string (dangerouslySetInnerHTML, innerHTML, outerHTML, insertAdjacentHTML, setHTMLUnsafe,
// srcdoc, document.write), nor code (eval, new Function), nor a regular expression built at run
// time (RegExp): what is typed is matched by a linear scan, never compiled. Scans .ts, .tsx,
// .js, .mjs, .css, .html. Fails with every offence, file:line.
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const RULES = [
  [
    /(?<![&\w])#(?:[0-9a-f]{8}|[0-9a-f]{6}|[0-9a-f]{3,4})(?![\w-])/i,
    'a raw hex colour: use a --mn-* token',
  ],
  [
    /(?<![\w-])(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/i,
    'a raw colour function: use a --mn-* token',
  ],
  [/dangerouslySetInnerHTML/, 'HTML from a string: render text nodes'],
  [/\b(?:inner|outer)HTML\b/, 'HTML from a string: render text nodes'],
  [/\binsertAdjacentHTML\b/, 'HTML from a string: render text nodes'],
  [/\bsetHTMLUnsafe\b/, 'HTML from a string: render text nodes'],
  [/\bsrcdoc\b/i, 'HTML from a string: render text nodes'],
  [/\bdocument\.write(?:ln)?\b/, 'HTML from a string: render text nodes'],
  [/\bnew\s+Function\b/, 'code from a string: never'],
  [/(?<![\w$])eval\s*\(/, 'code from a string: never'],
  [/\bRegExp\s*\(/, 'a regular expression built at run time: match by a scan'],
];

/** Comments out, line numbers kept: a rule's own explanation is not an offence. */
function stripComments(text) {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, ' '))
    .replace(/(^|[^:'"`])\/\/.*$/gm, '$1');
}

/** Every offence in one file's text, as `line: reason`. */
export function offences(text) {
  const out = [];
  stripComments(text)
    .split('\n')
    .forEach((line, i) => {
      for (const [re, why] of RULES) {
        if (re.test(line)) out.push(`${i + 1}: ${why}`);
      }
    });
  return out;
}

function* files(dir) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    if (e.name === 'node_modules' || e.name === 'dist' || e.name === 'test-results') continue;
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* files(p);
    else if (/\.(tsx?|m?js|css|html)$/.test(e.name)) yield p;
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const root = fileURLToPath(new URL('../packages/board/', import.meta.url));
  const errors = [];
  let n = 0;
  for (const f of files(root)) {
    n += 1;
    for (const o of offences(readFileSync(f, 'utf8')))
      errors.push(`${relative(process.cwd(), f)}:${o}`);
  }
  if (errors.length) {
    console.error(`board check failed:\n  ${errors.join('\n  ')}`);
    process.exit(1);
  }
  console.log(`board check: ${n} files, tokens only, no HTML from strings`);
}
