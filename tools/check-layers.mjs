// Enforces the crate layering of docs/05-architecture.md: a crate may depend only on crates in a
// lower layer. Fails with every offending edge.
import { execFileSync } from 'node:child_process';

const LAYERS = {
  'tempest-wire': 0,
  'tempest-races': 0,
  'tempest-sandbox': 0,
  'tempest-memory': 0,
  'tempest-ledger': 1,
  'tempest-tools': 1,
  'tempest-provider': 2,
  'tempest-world': 2,
  'tempest-loop': 3,
  'tempest-server': 4,
  tempestd: 5,
};

const meta = JSON.parse(
  execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], { encoding: 'utf8' }),
);
const errors = [];
for (const pkg of meta.packages) {
  const own = LAYERS[pkg.name];
  if (own === undefined) {
    errors.push(`${pkg.name}: not placed in a layer — add it to tools/check-layers.mjs`);
    continue;
  }
  for (const dep of pkg.dependencies) {
    const theirs = LAYERS[dep.name];
    if (theirs !== undefined && theirs >= own) {
      errors.push(`${pkg.name} (layer ${own}) depends on ${dep.name} (layer ${theirs})`);
    }
  }
}
if (errors.length) {
  console.error(`layer check failed:\n  ${errors.join('\n  ')}`);
  process.exit(1);
}
console.log(`layer check: ${meta.packages.length} crates, every edge points down`);
