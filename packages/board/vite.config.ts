/// <reference types="vitest/config" />
import react from '@vitejs/plugin-react';
import { defineConfig, type Plugin } from 'vite';
import { fixtureWorlds } from './src/data/fixtures.ts';

/** In `vite dev` only, `/api/worlds` answers with the fixtures: the board's server is B1's. */
function fixtures(): Plugin {
  return {
    name: 'mnemorph-board-fixtures',
    apply: 'serve',
    configureServer(server) {
      server.middlewares.use('/api/worlds', (_req, res) => {
        res.setHeader('content-type', 'application/json');
        res.end(JSON.stringify(fixtureWorlds));
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
