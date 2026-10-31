/// <reference types="vitest/config" />
import react from '@vitejs/plugin-react';
import { defineConfig, type Plugin } from 'vite';
import { fixtureBodies, fixtureQueue, journalOf, timelineOf } from './src/data/bodiesFixtures.ts';
import { fixtureOverviews, fixtureWorlds } from './src/data/fixtures.ts';
import { factsOf, fixtureMemory } from './src/data/knowledgeFixtures.ts';

/**
 * In `vite dev` only, `/api/worlds`, `/api/w/<id>` and its memory, facts, bodies, queue, timeline
 * and journal answer with the fixtures.
 */
function fixtures(): Plugin {
  return {
    name: 'mnemorph-board-fixtures',
    apply: 'serve',
    configureServer(server) {
      server.middlewares.use('/api/worlds', (_req, res) => {
        res.setHeader('content-type', 'application/json');
        res.end(JSON.stringify(fixtureWorlds));
      });
      server.middlewares.use('/api/w/', (req, res, next) => {
        const url = new URL(req.url ?? '/', 'http://board');
        const [, , page] = url.pathname.split('/');
        if (page === 'memory' || page === 'facts') {
          res.setHeader('content-type', 'application/json');
          res.end(
            JSON.stringify(
              page === 'memory' ? fixtureMemory : factsOf(url.searchParams.get('status')),
            ),
          );
          return;
        }
        const more: Record<string, () => unknown> = {
          bodies: () => fixtureBodies,
          queue: () => fixtureQueue,
          timeline: () => timelineOf(url.searchParams.get('kind'), url.searchParams.get('body')),
          journal: () => journalOf(url.searchParams.get('q')),
        };
        const answer = page === undefined ? undefined : more[page];
        if (answer) {
          res.setHeader('content-type', 'application/json');
          res.end(JSON.stringify(answer()));
          return;
        }
        if (page !== undefined) return next();
        const id = decodeURIComponent((req.url ?? '').replace(/^\//, ''));
        const o = fixtureOverviews[id];
        res.statusCode = o ? 200 : 404;
        res.setHeader('content-type', o ? 'application/json' : 'text/plain');
        res.end(o ? JSON.stringify(o) : 'no listed world has this id');
      });
    },
  };
}

/** The port `vite preview` serves the end-to-end tests on. */
export const PREVIEW_PORT = 4317;

/**
 * The headers `mnemorphd board` sends with every response (crates/mnemorph-server/src/board/
 * guard.rs), so the end-to-end tests run the page under the same CSP: a violation fails them.
 */
export const BOARD_HEADERS = {
  'content-security-policy':
    "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; " +
    `font-src 'self'; connect-src 'self' ws://127.0.0.1:${PREVIEW_PORT}; base-uri 'none'; ` +
    "form-action 'none'; frame-ancestors 'none'; require-trusted-types-for 'script'",
  'x-content-type-options': 'nosniff',
  'referrer-policy': 'no-referrer',
  'cross-origin-opener-policy': 'same-origin',
  'cross-origin-resource-policy': 'same-origin',
  'cache-control': 'no-store',
};

export default defineConfig({
  plugins: [react(), fixtures()],
  preview: { host: '127.0.0.1', port: PREVIEW_PORT, strictPort: true, headers: BOARD_HEADERS },
  build: { sourcemap: false },
  test: { include: ['src/**/*.test.{ts,tsx}'] },
});
