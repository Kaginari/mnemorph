#!/usr/bin/env bash
# The npm packages as a user gets them: built, packed, installed from their tarballs into an empty
# project with no workspace in reach, then imported. Run from the repository root.
set -euo pipefail
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
for p in protocol sdk; do
    pnpm --filter "@mnemorph/$p" build
    (cd "packages/$p" && pnpm pack --pack-destination "$scratch" >/dev/null)
done
tar -tzf "$scratch"/mnemorph-sdk-*.tgz | grep -q 'package/dist/index.d.ts'
! tar -tzf "$scratch"/mnemorph-sdk-*.tgz | grep -Eq '\.test\.|package/src/'
cd "$scratch"
npm init -y >/dev/null
npm install --silent --no-audit --no-fund ./mnemorph-protocol-*.tgz ./mnemorph-sdk-*.tgz
grep -q 'workspace:' node_modules/@mnemorph/sdk/package.json && { echo "workspace: left in the package"; exit 1; }
cat > check.mjs <<'JS'
import * as sdk from '@mnemorph/sdk';
import * as browser from '@mnemorph/sdk/browser';
import * as protocol from '@mnemorph/protocol';
const names = Object.keys(sdk);
if (names.length === 0 || Object.keys(browser).length === 0) throw new Error('nothing exported');
console.log('imported:', names.slice(0, 6).join(', '), '| protocol', protocol.PROTOCOL);
JS
node check.mjs
echo "npm-pack: PASS"
