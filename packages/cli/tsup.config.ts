import { defineConfig } from 'tsup';

// The workspace's own packages are TypeScript sources: they are bundled in, so `mnemorph` runs on
// plain Node; Ink and React stay dependencies.
export default defineConfig({
  entry: ['src/cli.tsx'],
  format: ['esm'],
  clean: true,
  noExternal: [/^@mnemorph\//],
});
