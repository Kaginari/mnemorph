// Draws the book's canvases: each sketch listed below is taken from its page's Mermaid block,
// laid out by tldraw's own converter (@tldraw/mermaid) in headless Chromium, and written as
// docs/canvas/<page>.tldr (one tldraw page per sketch) and docs/canvas/<sketch>-{light,dark}.svg.
// The Mermaid block stays the source of truth: change it, then run `pnpm --filter
// @mnemorph/canvas draw`. Needs the Chromium @playwright/test pins (`pnpm --filter
// @mnemorph/canvas exec playwright install chromium`).
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from '@playwright/test';
import { createServer } from 'vite';
import { finishSvg } from './finish.mjs';
import { flattenLabels } from './flatten.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const docs = join(here, '..', '..', 'docs');
const out = join(docs, 'canvas');

// page → its sketches: [the canvas's name, the Mermaid block's place on the page (0-based), title,
// the canvas's own layout]. A canvas must read in the book's ~900 px column, so a long or wide
// sketch is re-laid for its canvas only (its Mermaid text stays as written): `direction` turns a
// left-right flow top-down, `elk` lays it out with ELK (its options), `wrap` breaks a sequence's
// messages at that many characters, `sequence` sets Mermaid's sequence layout (an even participant
// margin, since tldraw stretches every gap by the narrowest's ratio, and its font size, so rows fit
// the labels), `labelSize` sets tldraw's label size, and `spacing` Mermaid's gaps between nodes
// and ranks.
const SKETCHES = {
  '01-overview': [['overview', 0, 'The overview']],
  '02-races': [['races', 0, 'Races and ascensions']],
  '03-loop': [['loop', 0, 'The loop', { direction: 'TD' }]],
  '04-communication': [
    [
      'order',
      0,
      'One order, down and back',
      { wrap: 14, sequence: { actorMargin: 120 }, labelSize: 'l', stretchY: 1.6 },
    ],
    ['client-protocol', 3, 'The client protocol'],
  ],
  '05-architecture': [
    ['architecture', 0, 'The architecture', { elk: { nodePlacementStrategy: 'SIMPLE' } }],
    ['sandbox', 1, 'The sandbox', { direction: 'TD' }],
  ],
  '06-consumption': [['consumption', 0, 'Consumption']],
};

// The narrowest a canvas's labels may print in the book's column, and its widest shape.
const COLUMN = 900;
const MIN_LABEL_PX = 10;
const MAX_ASPECT = 2;

// The nth Mermaid block of a page, its front matter (GitHub's look settings) left out.
function mermaidBlock(page, nth) {
  const text = readFileSync(join(docs, `${page}.md`), 'utf8');
  const blocks = [...text.matchAll(/```mermaid\n([\s\S]*?)```/g)].map((m) => m[1]);
  if (!blocks[nth]) throw new Error(`${page}.md has no Mermaid block #${nth}`);
  return blocks[nth].replace(/^---\n[\s\S]*?\n---\n/, '');
}

function wrapText(text, width) {
  const lines = [];
  for (const word of text.split(' ')) {
    const last = lines.at(-1);
    if (last !== undefined && `${last} ${word}`.length <= width)
      lines[lines.length - 1] = `${last} ${word}`;
    else lines.push(word);
  }
  return lines.join('<br/>');
}

// The sketch as its canvas lays it out.
function canvasLayout(
  mermaid,
  { direction, wrap, spacing, sequence, elk, labelSize, stretchY } = {},
) {
  let text = mermaid;
  if (direction) text = text.replace(/^(\s*flowchart\s+)(?:LR|RL|TB|TD|BT)\b/m, `$1${direction}`);
  if (wrap) {
    text = text.replace(
      /^(\s*\w+\s*[-=]+[>x)]+[+-]?\s*\w+\s*:\s*)(.+)$/gm,
      (_m, head, message) => head + wrapText(message, wrap),
    );
  }
  const mermaidConfig = {};
  if (spacing) mermaidConfig.flowchart = { nodeSpacing: spacing[0], rankSpacing: spacing[1] };
  if (sequence) mermaidConfig.sequence = sequence;
  if (elk) Object.assign(mermaidConfig, { layout: 'elk', elk: elk === true ? {} : elk });
  return { mermaid: text, mermaidConfig, labelSize, stretchY };
}

// The nodes a dashed classDef marks (tldraw's converter dashes only edges from their styles).
function dashedNodes(mermaid) {
  const dashed = new Set(
    [...mermaid.matchAll(/^\s*classDef\s+(\w+)\s+.*stroke-dasharray/gm)].map((m) => m[1]),
  );
  const nodes = [];
  for (const [, ids, name] of mermaid.matchAll(/^\s*class\s+([\w,\s]+?)\s+(\w+)\s*$/gm)) {
    if (dashed.has(name)) nodes.push(...ids.split(',').map((id) => id.trim()));
  }
  return nodes;
}

// A canvas too wide for the column prints its labels too small to read.
function legibility(name, svg) {
  const width = Number(svg.match(/<svg[^>]*\swidth="([\d.]+)"/)[1]);
  const height = Number(svg.match(/<svg[^>]*\sheight="([\d.]+)"/)[1]);
  const sizes = [...svg.matchAll(/font-size="([\d.]+)"/g)].map((m) => Number(m[1]));
  const counts = new Map();
  for (const size of sizes) counts.set(size, (counts.get(size) ?? 0) + 1);
  const label = [...counts].sort((a, b) => b[1] - a[1])[0][0];
  const printed = (label * Math.min(COLUMN, width)) / width;
  const problems = [];
  if (width / height > MAX_ASPECT) problems.push(`${(width / height).toFixed(1)}:1 wide`);
  if (printed < MIN_LABEL_PX) problems.push(`labels print at ${printed.toFixed(1)} px`);
  return problems.length ? `${name}: ${problems.join(', ')}` : undefined;
}

function serveAssets(dir) {
  return {
    name: 'tldraw-assets',
    configureServer(server) {
      server.middlewares.use('/tldraw-assets', (req, res, next) => {
        const path = decodeURIComponent(new URL(req.url, 'http://x').pathname);
        if (path.includes('..')) return next();
        try {
          const body = readFileSync(join(dir, path));
          const type = {
            '.woff2': 'font/woff2',
            '.svg': 'image/svg+xml',
            '.json': 'application/json',
          };
          res.setHeader(
            'content-type',
            type[path.slice(path.lastIndexOf('.'))] ?? 'application/octet-stream',
          );
          res.end(body);
        } catch {
          next();
        }
      });
    },
  };
}

const server = await createServer({
  root: here,
  logLevel: 'error',
  // @tldraw/assets' fonts, icons and translations, served at /tldraw-assets.
  plugins: [serveAssets(join(here, 'node_modules', '@tldraw', 'assets'))],
  server: { host: '127.0.0.1', port: 0, strictPort: false },
});
await server.listen();
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
  page.on('pageerror', (e) => console.error('page error:', e.message));
  // Nothing leaves loopback: a font or icon asked of a CDN is refused and fails the draw.
  const blocked = [];
  const illegible = [];
  await page.route(/^(?!http:\/\/127\.0\.0\.1[:/])/, (route) => {
    blocked.push(route.request().url());
    return route.abort();
  });
  await page.goto(server.resolvedUrls.local[0]);
  // Each SVG is flattened alone, as an image viewer would open it.
  const lone = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
  await lone.route(/^(?!data:)/, (route) => route.abort());
  await page.waitForFunction(() => window.mnEditor && window.mnDraw);
  for (const [name, sketches] of Object.entries(SKETCHES)) {
    const input = sketches.map(([, nth, title, layout]) => {
      const source = mermaidBlock(name, nth);
      return { title, ...canvasLayout(source, layout), dashed: dashedNodes(source) };
    });
    const result = await page.evaluate((s) => window.mnDraw(s), input);
    writeFileSync(join(out, `${name}.tldr`), `${result.tldr}\n`);
    for (const [i, [sketch, , title]] of sketches.entries()) {
      for (const theme of ['light', 'dark']) {
        await lone.setContent(
          `<!doctype html><body style="margin:0">${result.svgs[i][theme]}</body>`,
        );
        await lone.evaluate(() => document.fonts.ready);
        const flat = await lone.evaluate(flattenLabels);
        const svg = await finishSvg(flat, title);
        if (theme === 'light') {
          const problem = legibility(sketch, svg);
          if (problem) illegible.push(problem);
        }
        writeFileSync(join(out, `${sketch}-${theme}.svg`), svg);
      }
    }
    console.log(`drew ${name}: ${sketches.map(([s]) => s).join(', ')}`);
  }
  if (illegible.length) throw new Error(`too small to read:\n  ${illegible.join('\n  ')}`);
  if (blocked.length) throw new Error(`the draw asked outside loopback: ${blocked.join(', ')}`);
} finally {
  await browser.close();
  await server.close();
}
