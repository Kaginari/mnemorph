#!/bin/sh
# Install Mnemorph — `mnemorph`, `mnemorphd` and, on Linux, `mnemorph-guard` — from a GitHub
# release, for this machine's OS and architecture.
#
#   curl -fsSL https://raw.githubusercontent.com/Kaginari/mnemorph/main/install.sh | sh
#   sh install.sh --version 1.0.0 --dir ~/bin
#
# The archive is checked against the release's SHA256SUMS before anything is unpacked: a
# missing checksum file, a missing line or a mismatch stops the install. Nothing needs sudo,
# and no shell profile is edited: when the install directory is not on PATH, the line to add is
# printed instead.
#
# Environment:
#   MNEMORPH_VERSION       the version to install (default: the latest release)
#   MNEMORPH_INSTALL_DIR   where the binaries go (default: ~/.local/bin)
#   MNEMORPH_RELEASE_BASE  where releases are read from
#                          (default: https://github.com/Kaginari/mnemorph/releases)
set -eu

base=${MNEMORPH_RELEASE_BASE:-https://github.com/Kaginari/mnemorph/releases}
version=${MNEMORPH_VERSION:-latest}
dir=${MNEMORPH_INSTALL_DIR:-}

say() { printf 'mnemorph-install: %s\n' "$*"; }
die() { printf 'mnemorph-install: error: %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'EOF'
Install Mnemorph from a GitHub release.

Usage: install.sh [--version <x.y.z|latest>] [--dir <install dir>]

  --version, -v   the release to install (default: latest; MNEMORPH_VERSION)
  --dir, -d       where to put the binaries (default: ~/.local/bin; MNEMORPH_INSTALL_DIR)
  --help, -h      this help

The archive is verified against the release's SHA256SUMS before it is unpacked.
No sudo, and no shell profile is changed.
EOF
}

while [ $# -gt 0 ]; do
  case $1 in
    -v | --version) [ $# -ge 2 ] || die "$1 needs a value"; version=$2; shift 2 ;;
    -d | --dir) [ $# -ge 2 ] || die "$1 needs a value"; dir=$2; shift 2 ;;
    -h | --help) usage; exit 0 ;;
    *) die "unknown argument '$1' (see --help)" ;;
  esac
done

if [ -z "$dir" ]; then
  [ -n "${HOME:-}" ] || die "HOME is unset: pass --dir or set MNEMORPH_INSTALL_DIR"
  dir=$HOME/.local/bin
fi
[ "$(id -u)" -ne 0 ] || say "running as root: nothing here needs it"

# ---- this machine --------------------------------------------------------------------------------
os=$(uname -s)
arch=$(uname -m)
case $arch in
  x86_64 | amd64) arch=x86_64 ;;
  aarch64 | arm64) arch=aarch64 ;;
  *) die "no release for architecture $arch (build from source: see the README)" ;;
esac
case $os in
  Linux) target=$arch-unknown-linux-gnu ;;
  Darwin)
    target=$arch-apple-darwin
    # A shell under Rosetta reports x86_64 on an Apple Silicon machine: install the native build.
    if [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 1 ]; then
      target=aarch64-apple-darwin
    fi
    ;;
  *) die "no release for $os (build from source: see the README)" ;;
esac

# ---- tools ---------------------------------------------------------------------------------------
# HTTPS only, redirects included, unless the release base was overridden (a mirror or a test).
proto='=https'
[ -z "${MNEMORPH_RELEASE_BASE:-}" ] || proto='=https,http,file'
if command -v curl >/dev/null 2>&1; then
  fetch() { curl --proto "$proto" --proto-redir "$proto" -fsSL --retry 3 -o "$2" "$1"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -q -O "$2" "$1"; }
else
  die "needs curl or wget"
fi
if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d ' ' -f 1; }
elif command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
elif command -v openssl >/dev/null 2>&1; then
  sha256() { openssl dgst -sha256 -r "$1" | cut -d ' ' -f 1; }
else
  die "needs sha256sum, shasum or openssl to verify the download"
fi
command -v tar >/dev/null 2>&1 || die "needs tar"

# ---- download and verify -------------------------------------------------------------------------
case $version in
  latest) url=$base/latest/download ;;
  v*) url=$base/download/$version ;;
  *) url=$base/download/v$version ;;
esac
asset=mnemorph-$target.tar.gz

tmp=$(mktemp -d 2>/dev/null || mktemp -d -t mnemorph-install)
trap 'rm -rf "$tmp"' EXIT INT TERM

say "installing $version for $target into $dir"
fetch "$url/SHA256SUMS" "$tmp/SHA256SUMS" \
  || die "no SHA256SUMS at $url: refusing to install an unverified archive"
fetch "$url/$asset" "$tmp/$asset" || die "could not download $url/$asset"

# The line for this archive: "<64 hex>  <name>" (or "*<name>", binary mode).
want=$(awk -v f="$asset" '$2 == f || $2 == "*" f { print $1; exit }' "$tmp/SHA256SUMS")
[ -n "$want" ] || die "SHA256SUMS has no line for $asset: refusing to install it"
printf '%s' "$want" | grep -Eq '^[0-9a-fA-F]{64}$' || die "SHA256SUMS' line for $asset is not a SHA-256"
got=$(sha256 "$tmp/$asset")
want=$(printf '%s' "$want" | tr 'A-F' 'a-f')
[ "$got" = "$want" ] || die "checksum mismatch for $asset (want $want, got $got): refusing to install it"
say "verified $asset (sha256 $got)"

# ---- unpack and install --------------------------------------------------------------------------
mkdir "$tmp/x"
tar -xzf "$tmp/$asset" -C "$tmp/x"
src=$(find "$tmp/x" -type f -name mnemorph | head -n 1)
[ -n "$src" ] || die "the archive holds no mnemorph binary"
src=$(dirname "$src")
bins="mnemorph mnemorphd"
[ "$os" = Linux ] && bins="$bins mnemorph-guard"
for b in $bins; do
  [ -f "$src/$b" ] || die "the archive holds no $b"
done

mkdir -p "$dir"
for b in $bins; do
  # Copied beside, then renamed over: a running binary is never written in place.
  cp "$src/$b" "$dir/.$b.new"
  chmod 755 "$dir/.$b.new"
  mv -f "$dir/.$b.new" "$dir/$b"
done
say "installed: $(echo "$bins" | sed 's/ /, /g')"
installed=$("$dir/mnemorph" --version 2>/dev/null) || die "the installed mnemorph does not run on this machine"
say "$installed"

case ":${PATH:-}:" in
  *":$dir:"*) say "run: mnemorph status" ;;
  *)
    say "$dir is not on your PATH. Add it, for example:"
    # The literal line to paste: $PATH stays unexpanded.
    # shellcheck disable=SC2016
    printf '\n    export PATH="%s:$PATH"\n\n' "$dir"
    ;;
esac
