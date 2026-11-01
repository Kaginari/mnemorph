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

// page → its sketches: [the canvas's name, the Mermaid block's place on the page (0-based), title]
const SKETCHES = {
  '01-overview': [['overview', 0, 'The overview']],
  '02-races': [['races', 0, 'Races and ascensions']],
  '03-loop': [['loop', 0, 'The loop']],
  '04-communication': [
    ['order', 0, 'One order, down and back'],
    ['client-protocol', 3, 'The client protocol'],
  ],
  '05-architecture': [
    ['architecture', 0, 'The architecture'],
    ['sandbox', 1, 'The sandbox'],
  ],
  '06-consumption': [['consumption', 0, 'Consumption']],
};

// The nth Mermaid block of a page, its front matter (GitHub's look settings) left out.
function mermaidBlock(page, nth) {
  const text = readFileSync(join(docs, `${page}.md`), 'utf8');
  const blocks = [...text.matchAll(/```mermaid\n([\s\S]*?)```/g)].map((m) => m[1]);
  if (!blocks[nth]) throw new Error(`${page}.md has no Mermaid block #${nth}`);
  return blocks[nth].replace(/^---\n[\s\S]*?\n---\n/, '');
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
    const input = sketches.map(([, nth, title]) => ({ title, mermaid: mermaidBlock(name, nth) }));
    const result = await page.evaluate((s) => window.mnDraw(s), input);
    writeFileSync(join(out, `${name}.tldr`), `${result.tldr}\n`);
    for (const [i, [sketch, , title]] of sketches.entries()) {
      for (const theme of ['light', 'dark']) {
        await lone.setContent(
          `<!doctype html><body style="margin:0">${result.svgs[i][theme]}</body>`,
        );
        await lone.evaluate(() => document.fonts.ready);
        const flat = await lone.evaluate(flattenLabels);
        writeFileSync(join(out, `${sketch}-${theme}.svg`), await finishSvg(flat, title));
      }
    }
    console.log(`drew ${name}: ${sketches.map(([s]) => s).join(', ')}`);
  }
  if (blocked.length) throw new Error(`the draw asked outside loopback: ${blocked.join(', ')}`);
} finally {
  await browser.close();
  await server.close();
}
