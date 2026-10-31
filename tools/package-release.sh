#!/usr/bin/env bash
# Package one target's release binaries as the archive install.sh downloads:
# `mnemorph-<target>.tar.gz`, holding `mnemorph-<version>-<target>/` with the binaries (the guard
# too, on Linux), the licence, the third-party notices, the README and the changelog.
#
#   tools/package-release.sh <bin-dir> <target> <version> <out-dir>
#
# Prints the archive's path. The release's SHA256SUMS is written over every archive at once.
set -euo pipefail

[ $# -eq 4 ] || { echo "usage: tools/package-release.sh <bin-dir> <target> <version> <out-dir>" >&2; exit 2; }
bin=$1 target=$2 version=${3#v} out=$4
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

bins=(mnemorph mnemorphd)
case $target in *-linux-*) bins+=(mnemorph-guard) ;; esac

name=mnemorph-$version-$target
stage=$(mktemp -d "${TMPDIR:-/tmp}/mnemorph-package.XXXXXX")
trap 'rm -rf "$stage"' EXIT
mkdir "$stage/$name"
for b in "${bins[@]}"; do
  [ -x "$bin/$b" ] || { echo "package-release: $bin/$b is missing" >&2; exit 1; }
  install -m 755 "$bin/$b" "$stage/$name/$b"
done
for f in LICENSE THIRD-PARTY-NOTICES.md README.md CHANGELOG.md; do
  install -m 644 "$root/$f" "$stage/$name/$f"
done

mkdir -p "$out"
archive=$(cd "$out" && pwd)/mnemorph-$target.tar.gz
tar -czf "$archive" -C "$stage" "$name"
echo "$archive"
