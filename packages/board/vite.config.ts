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

export default defineConfig({
  plugins: [react(), fixtures()],
  build: { sourcemap: false },
  test: { include: ['src/**/*.test.{ts,tsx}'] },
});
