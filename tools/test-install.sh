#!/usr/bin/env bash
# Scripts and dotfiles are written with their $ variables unexpanded.
# shellcheck disable=SC2016
#
# The installer, offline: a fake release (an archive per target and SHA256SUMS) is served from a
# local directory on loopback, and install.sh is pointed at it through MNEMORPH_RELEASE_BASE.
# A fake `uname` on PATH plays each supported machine. The fresh install is timed against the
# M14 exit (under two minutes), and every way a download can be wrong is refused.
#
#   tools/test-install.sh [archive]
#
# With a release archive built for this machine (tools/package-release.sh), the fresh install is
# that real archive, run once installed; without one, every archive holds stand-in binaries.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
installer=$here/../install.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/mnemorph-install-test.XXXXXX")
server_pid=
cleanup() {
  if [ -n "$server_pid" ]; then kill "$server_pid" 2>/dev/null || true; fi
  rm -rf "$tmp"
}
trap cleanup EXIT

fails=0
pass() { printf '  ok    %s\n' "$1"; }
fail() { printf '  FAIL  %s\n' "$1" >&2; fails=$((fails + 1)); }
sha() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d ' ' -f 1; }

real=${1:-}
if [ -n "$real" ]; then real=$(cd "$(dirname "$real")" && pwd)/$(basename "$real"); fi
version=1.2.3
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) host=x86_64-unknown-linux-gnu ;; Linux-aarch64) host=aarch64-unknown-linux-gnu ;;
  Darwin-arm64) host=aarch64-apple-darwin ;; Darwin-x86_64) host=x86_64-apple-darwin ;;
  *) host=x86_64-unknown-linux-gnu; [ -z "$real" ] || { echo "no release target for this machine" >&2; exit 2; } ;;
esac
targets="x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin"

# ---- a fake release ------------------------------------------------------------------------------
rel=$tmp/releases
publish() { # dir, version
  local out=$1 v=$2 t stage
  mkdir -p "$out"
  for t in $targets; do
    if [ -n "$real" ] && [ "$t" = "$host" ] && [ "$v" = "$version" ]; then
      cp "$real" "$out/mnemorph-$t.tar.gz"
      continue
    fi
    stage=$tmp/stage/mnemorph-$v-$t
    mkdir -p "$stage"
    for b in mnemorph mnemorphd; do
      printf '#!/bin/sh\necho "%s %s (%s)"\n' "$b" "$v" "$t" > "$stage/$b"
      chmod +x "$stage/$b"
    done
    case $t in *linux*)
      printf '#!/bin/sh\nexit 0\n' > "$stage/mnemorph-guard"; chmod +x "$stage/mnemorph-guard" ;;
    esac
    printf 'MIT\n' > "$stage/LICENSE"
    printf '# Mnemorph\n' > "$stage/README.md"
    tar -czf "$out/mnemorph-$t.tar.gz" -C "$tmp/stage" "mnemorph-$v-$t"
    rm -rf "$tmp/stage"
  done
  (cd "$out" && for f in mnemorph-*.tar.gz; do printf '%s  %s\n' "$(sha "$f")" "$f"; done > SHA256SUMS)
}
publish "$rel/download/v$version" "$version"
publish "$rel/download/v1.0.0" 1.0.0
mkdir -p "$rel/latest"
cp -R "$rel/download/v$version" "$rel/latest/download"
# Broken releases: one archive altered after its sum was written, one with no SHA256SUMS, one
# whose SHA256SUMS lacks the archive's line.
cp -R "$rel/download/v$version" "$rel/download/v6.6.6"
for t in $targets; do printf 'tampered' >> "$rel/download/v6.6.6/mnemorph-$t.tar.gz"; done
cp -R "$rel/download/v$version" "$rel/download/v7.7.7"
rm "$rel/download/v7.7.7/SHA256SUMS"
cp -R "$rel/download/v$version" "$rel/download/v8.8.8"
: > "$rel/download/v8.8.8/SHA256SUMS"

# ---- served from that directory on loopback ------------------------------------------------------
python3 -c '
import functools, http.server, os, sys
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=sys.argv[1])
handler.func.log_message = lambda *a: None
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
open(sys.argv[2] + ".tmp", "w").write(str(server.server_address[1]))
os.replace(sys.argv[2] + ".tmp", sys.argv[2])
server.serve_forever()
' "$rel" "$tmp/port" &
server_pid=$!
for _ in $(seq 1 100); do [ -s "$tmp/port" ] && break; sleep 0.05; done
[ -s "$tmp/port" ] || { echo "the release server did not start" >&2; exit 1; }
base=http://127.0.0.1:$(cat "$tmp/port")

# A machine: a fresh HOME with a .bashrc, and a `uname` that says what it is.
machine() { # name, os, arch
  local m=$tmp/m/$1
  mkdir -p "$m/home" "$m/fakebin"
  printf '# ~/.bashrc\nexport PATH="$HOME/bin:$PATH"\n' > "$m/home/.bashrc"
  printf '#!/bin/sh\ncase "$1" in -s) echo %s ;; -m) echo %s ;; *) echo %s ;; esac\n' "$2" "$3" "$2" > "$m/fakebin/uname"
  chmod +x "$m/fakebin/uname"
  echo "$m"
}
install_on() { # machine dir, then installer arguments; MNEMORPH_* from the caller's environment
  local m=$1; shift
  env -i "HOME=$m/home" "PATH=$m/fakebin:/usr/bin:/bin" "MNEMORPH_RELEASE_BASE=$base" \
    ${MNEMORPH_VERSION:+"MNEMORPH_VERSION=$MNEMORPH_VERSION"} \
    ${MNEMORPH_INSTALL_DIR:+"MNEMORPH_INSTALL_DIR=$MNEMORPH_INSTALL_DIR"} \
    sh "$installer" "$@" > "$m/out" 2>&1
}

echo "installer: $installer (served from $base)"

# ---- the fresh install, timed --------------------------------------------------------------------
host_os=Linux; host_arch=${host%%-*}
case $host in *darwin*) host_os=Darwin; [ "$host_arch" = aarch64 ] && host_arch=arm64 ;; esac
m=$(machine fresh "$host_os" "$host_arch")
bashrc_before=$(cksum < "$m/home/.bashrc")
start=$(date +%s)
if install_on "$m"; then pass "a fresh install of the latest release"; else fail "a fresh install: $(cat "$m/out")"; fi
# Installed means running: the first command the human types, inside the timed path.
installed=$(env -i "HOME=$m/home" PATH=/usr/bin:/bin "$m/home/.local/bin/mnemorph" --version 2>&1) || installed="failed: $installed"
elapsed=$(( $(date +%s) - start ))
if [ "$elapsed" -lt 120 ]; then pass "installed and running in ${elapsed}s (the exit: under 120s)"
else fail "the fresh install took ${elapsed}s (the exit: under 120s)"; fi
bins="mnemorph mnemorphd"
case $host in *linux*) bins="$bins mnemorph-guard" ;; esac
for b in $bins; do
  if [ -x "$m/home/.local/bin/$b" ]; then pass ".local/bin/$b in the fresh HOME"; else fail ".local/bin/$b is missing from the fresh HOME"; fi
done
if [ -n "$real" ]; then
  case $installed in "mnemorph "*) pass "the real build runs: $installed" ;; *) fail "the real build runs: $installed" ;; esac
elif [ "$installed" = "mnemorph $version ($host)" ]; then
  pass "the latest is $version, the $host build"
else fail "installed: $installed"; fi
if grep -qF "export PATH=\"$m/home/.local/bin:\$PATH\"" "$m/out"; then pass "the PATH hint is printed"
else fail "the PATH hint is printed: $(cat "$m/out")"; fi
if [ "$(cksum < "$m/home/.bashrc")" = "$bashrc_before" ] \
  && [ -z "$(cd "$m/home" && find . -maxdepth 1 -type f ! -name .bashrc)" ]; then
  pass "no shell profile written or changed"
else fail "a shell profile was written or changed"; fi
if grep -q "verified mnemorph-$host.tar.gz" "$m/out"; then pass "the checksum was verified"
else fail "the checksum was verified: $(cat "$m/out")"; fi
# Again over itself: an upgrade in place.
if MNEMORPH_VERSION=1.0.0 install_on "$m" && [ "$("$m/home/.local/bin/mnemorph" --version)" = "mnemorph 1.0.0 ($host)" ]; then
  pass "reinstalled over itself at another version"
else fail "reinstall: $(cat "$m/out")"; fi

# ---- every supported machine gets its own build --------------------------------------------------
for spec in "Linux aarch64 aarch64-unknown-linux-gnu" "Linux arm64 aarch64-unknown-linux-gnu" \
  "Darwin arm64 aarch64-apple-darwin" "Darwin x86_64 x86_64-apple-darwin"; do
  # shellcheck disable=SC2086 # split into its three words on purpose
  set -- $spec
  m=$(machine "$1-$2" "$1" "$2")
  if install_on "$m" --version "$version" --dir "$m/home/bin" \
    && [ "$("$m/home/bin/mnemorph" --version)" = "mnemorph $version ($3)" ]; then
    pass "$1 $2 installs $3"
  else fail "$1 $2 installs $3: $(cat "$m/out")"; fi
  case $1 in Darwin)
    if [ ! -e "$m/home/bin/mnemorph-guard" ]; then pass "no guard on macOS (the Seatbelt sandbox)"
    else fail "a guard was installed on macOS"; fi ;;
  esac
done
m=$(machine envdir Linux x86_64)
if MNEMORPH_INSTALL_DIR=$m/home/tools MNEMORPH_VERSION=v$version install_on "$m" && [ -x "$m/home/tools/mnemorphd" ]; then
  pass "MNEMORPH_INSTALL_DIR and MNEMORPH_VERSION are honoured"
else fail "MNEMORPH_INSTALL_DIR: $(cat "$m/out")"; fi

# ---- refusals ------------------------------------------------------------------------------------
refused() { # name, wanted message, version (or -), os, arch
  local m; m=$(machine "$1" "$4" "$5")
  local args=()
  [ "$3" = - ] || args=(--version "$3")
  if install_on "$m" ${args[@]+"${args[@]}"}; then fail "$1: installed, wanted a refusal"; return; fi
  if ! grep -qF "$2" "$m/out"; then fail "$1: refused without saying \`$2\`: $(cat "$m/out")"; return; fi
  if [ -n "$(ls -A "$m/home/.local/bin" 2>/dev/null)" ]; then fail "$1: refused, but left files in ~/.local/bin"; return; fi
  pass "$1 is refused, nothing installed"
}
refused "a checksum mismatch" "checksum mismatch" 6.6.6 Linux x86_64
refused "a missing SHA256SUMS" "no SHA256SUMS" 7.7.7 Linux x86_64
refused "a SHA256SUMS without the archive's line" "has no line for" 8.8.8 Linux x86_64
refused "a release that does not exist" "no SHA256SUMS" 9.9.9 Linux x86_64
refused "an unsupported OS" "no release for FreeBSD" - FreeBSD x86_64
refused "an unsupported architecture" "no release for architecture riscv64" - Linux riscv64

echo
if [ "$fails" -eq 0 ]; then echo "test-install: PASS"; else echo "test-install: FAIL ($fails)" >&2; exit 1; fi
