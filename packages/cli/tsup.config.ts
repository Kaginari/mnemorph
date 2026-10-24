import { execSync } from 'node:child_process';
import { defineConfig } from 'tsup';

function commit(): string {
  try {
    return execSync('git rev-parse --short=10 HEAD', { encoding: 'utf8' }).trim() || 'unknown';
  } catch {
    return 'unknown';
  }
}

// The workspace's own packages are TypeScript sources: they are bundled in, so `mnemorph` runs on
// plain Node; Ink and React stay dependencies.
export default defineConfig({
  entry: ['src/cli.tsx'],
  format: ['esm'],
  clean: true,
  noExternal: [/^@mnemorph\//],
  define: { __MNEMORPH_COMMIT__: JSON.stringify(commit()) },
});
