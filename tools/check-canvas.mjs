// The book's canvases are reviewed like code: a tldraw document or an SVG can carry script. Every
// file in docs/canvas/ must be a .tldr (tldraw's JSON document) or an .svg, and neither may hold a
// script, an event handler, a javascript: (or vbscript:, data:text/html) URL, a link or source
// outside the file, a <foreignObject>, or anything fetched at render time. An SVG may use only
// drawing elements, local #references and data: fonts; a .tldr only document records and drawn
// shapes (no embeds, bookmarks, images or videos, which load from elsewhere). A .tldraw archive is
// refused: it can embed a document script that runs when opened. The rules are proven on hostile
// samples before the canvases are read. Fails with every offence, file: reason.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const SVG_ELEMENTS = new Set([
  'svg',
  'g',
  'defs',
  'style',
  'title',
  'desc',
  'path',
  'rect',
  'circle',
  'ellipse',
  'line',
  'polyline',
  'polygon',
  'text',
  'tspan',
  'clipPath',
  'mask',
  'pattern',
  'marker',
  'linearGradient',
  'radialGradient',
  'stop',
  'use',
]);
const RECORDS = new Set(['document', 'page', 'shape', 'binding', 'asset']);
const SHAPES = new Set(['geo', 'arrow', 'line', 'group', 'text', 'frame', 'note', 'draw']);
const HOSTILE_URL = /(?:java|vb)script\s*:|data\s*:\s*text\/html/i;

function svgOffences(text) {
  const found = [];
  if (HOSTILE_URL.test(text)) found.push('a script URL');
  if (/<!DOCTYPE|<!ENTITY/i.test(text)) found.push('a DOCTYPE or entity declaration');
  if (/<\?xml-stylesheet/i.test(text)) found.push('an external stylesheet');
  for (const [, name, attrs] of text.matchAll(/<([a-zA-Z][\w:.-]*)([^>]*)>/g)) {
    if (!SVG_ELEMENTS.has(name)) found.push(`a <${name}> element`);
    for (const [, attr, value] of attrs.matchAll(/([\w:.-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/g)) {
      if (/^on/i.test(attr)) found.push(`an event handler (${attr})`);
      if (/^(?:xlink:)?href$|^src$/i.test(attr) && !/^#[\w-]+$/.test(value ?? '')) {
        found.push(`a link outside the file (${attr}="${value}")`);
      }
    }
  }
  for (const [, ref] of text.matchAll(/url\(\s*(?:["']|&quot;|&#3[49];)?([^"')&]*)/gi)) {
    if (!/^#[\w-]+$|^data:font\/woff2;base64,[A-Za-z0-9+/=]+$/.test(ref)) {
      found.push(`a url() outside the file (${ref.slice(0, 40)})`);
    }
  }
  if (/@import/i.test(text)) found.push('a CSS @import');
  return found;
}

function tldrOffences(text) {
  let file;
  try {
    file = JSON.parse(text);
  } catch {
    return ['not JSON'];
  }
  const found = [];
  if (typeof file.tldrawFileFormatVersion !== 'number' || !Array.isArray(file.records)) {
    found.push('not a tldraw document');
    return found;
  }
  const walk = (value, path) => {
    if (typeof value === 'string') {
      if (HOSTILE_URL.test(value)) found.push(`a script URL at ${path}`);
      if (/<\s*script/i.test(value)) found.push(`a script at ${path}`);
      if (/(?:^|\.)(?:href|src|url|\w*Url)$/i.test(path) && value !== '') {
        found.push(`a link at ${path}`);
      }
    } else if (value && typeof value === 'object') {
      for (const [k, v] of Object.entries(value)) {
        if (/script/i.test(k)) found.push(`a script field at ${path}.${k}`);
        if (/^on[A-Z]/.test(k)) found.push(`an event handler at ${path}.${k}`);
        walk(v, `${path}.${k}`);
      }
    }
  };
  for (const [i, record] of file.records.entries()) {
    if (!RECORDS.has(record.typeName)) found.push(`a ${record.typeName} record`);
    if (record.typeName === 'shape' && !SHAPES.has(record.type)) {
      found.push(`a ${record.type} shape`);
    }
    if (record.typeName === 'asset') found.push('an asset (media loads from elsewhere)');
    walk(record, `records[${i}]`);
  }
  for (const key of Object.keys(file)) {
    if (!['tldrawFileFormatVersion', 'schema', 'records'].includes(key)) {
      found.push(`an unexpected top-level field (${key})`);
    }
  }
  return found;
}

export function offences(name, text) {
  if (name.endsWith('.svg')) return svgOffences(text);
  if (name.endsWith('.tldr')) return tldrOffences(text);
  if (name.endsWith('.tldraw')) {
    return ['a .tldraw archive (it can embed a document script): commit the .tldr'];
  }
  return ['not a canvas (.tldr or .svg)'];
}

// Each rule, proven on a sample it must refuse, and a clean pair it must pass.
const doc = (records) => JSON.stringify({ tldrawFileFormatVersion: 1, schema: {}, records });
const shape = (props, type = 'geo') => ({ typeName: 'shape', id: 'shape:1', type, props });
const HOSTILE = [
  ['a.svg', '<svg><script>alert(1)</script></svg>'],
  ['a.svg', '<svg><g onload="alert(1)"/></svg>'],
  ['a.svg', '<svg><rect ONCLICK="x()"/></svg>'],
  ['a.svg', '<svg><a href="javascript:alert(1)"><text>x</text></a></svg>'],
  ['a.svg', '<svg><use href="https://example.com/x.svg#a"/></svg>'],
  ['a.svg', '<svg><use xlink:href="//example.com/x.svg#a"/></svg>'],
  [
    'a.svg',
    '<svg><foreignObject><div xmlns="http://www.w3.org/1999/xhtml">x</div></foreignObject></svg>',
  ],
  ['a.svg', '<svg><image href="data:image/png;base64,AA=="/></svg>'],
  ['a.svg', '<svg><style>@font-face{src:url(https://example.com/f.woff2)}</style></svg>'],
  ['a.svg', '<svg><g clip-path="url(&quot;https://example.com/a.svg#c&quot;)"/></svg>'],
  ['a.svg', '<svg><style>@import "https://example.com/a.css";</style></svg>'],
  ['a.svg', '<svg><animate attributeName="href" to="javascript:alert(1)"/></svg>'],
  ['a.svg', '<!DOCTYPE svg [<!ENTITY x "y">]><svg/>'],
  ['a.tldr', doc([shape({ url: 'https://example.com' }, 'embed')])],
  ['a.tldr', doc([shape({ text: '<script>alert(1)</script>' })])],
  ['a.tldr', doc([shape({ text: 'javascript:alert(1)' })])],
  [
    'a.tldr',
    doc([{ typeName: 'asset', id: 'asset:1', props: { src: 'https://example.com/a.png' } }]),
  ],
  ['a.tldr', doc([{ typeName: 'script', id: 'script:1', source: 'alert(1)' }])],
  ['a.tldr', `${doc([]).slice(0, -1)},"script":"alert(1)"}`],
  ['a.tldr', 'not json'],
  ['a.tldraw', 'PK'],
  ['a.html', '<p>x</p>'],
];
const CLEAN = [
  [
    'a.svg',
    '<svg viewBox="0 0 1 1"><style>@font-face{src:url("data:font/woff2;base64,AAAA") format("woff2")}</style><defs><clipPath id="c0"><rect/></clipPath></defs><g clip-path="url(#c0)"><path d="M0 0"/><text x="1" y="1">one, then on</text></g></svg>',
  ],
  ['a.tldr', doc([{ typeName: 'document', id: 'document:document' }, shape({ text: 'one' })])],
];
const unproven = [
  ...HOSTILE.filter(([name, text]) => offences(name, text).length === 0).map(
    ([name, text]) => `passes a hostile ${name}: ${text.slice(0, 70)}`,
  ),
  ...CLEAN.filter(([name, text]) => offences(name, text).length > 0).map(
    ([name, text]) => `refuses a clean ${name}: ${offences(name, text).join('; ')}`,
  ),
];
if (unproven.length) {
  console.error(`canvas check: a rule does not hold:\n  ${unproven.join('\n  ')}`);
  process.exit(1);
}

const dir = join('docs', 'canvas');
const failures = [];
let count = 0;
for (const name of readdirSync(dir).sort()) {
  count++;
  for (const reason of new Set(offences(name, readFileSync(join(dir, name), 'utf8')))) {
    failures.push(`${dir}/${name}: ${reason}`);
  }
}
if (failures.length) {
  console.error(`canvas check failed:\n  ${failures.join('\n  ')}`);
  process.exit(1);
}
console.log(`canvas check: ${count} canvases hold no script, handler, outside link or HTML`);
