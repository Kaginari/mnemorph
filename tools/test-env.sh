#!/usr/bin/env bash
# Run a test command (`cargo nextest run …`, `cargo test …`, `cargo llvm-cov …`) under a sentinel
# home: HOME and every XDG directory point into a fresh temporary directory holding a marker
# file, so nothing the tests do can reach the developer's real config or state. Cargo and rustup
# keep their own homes. Afterwards the sentinel must read exactly as before — every path, mtime
# and content hash — or the run fails: a test wrote to the home it was started under, which on a
# developer's machine is their real `~/.local/state/mnemorph`. (nextest gives the tests a home of
# their own, `tools/test-home.sh`; the sentinel is what they must never touch.)
#
#   tools/test-env.sh cargo nextest run --workspace
set -euo pipefail
[ $# -gt 0 ] || { echo "usage: tools/test-env.sh <command> [args…]" >&2; exit 2; }

sentinel=$(mktemp -d "${TMPDIR:-/tmp}/mnemorph-sentinel.XXXXXX")
trap 'rm -rf "$sentinel"' EXIT
for d in home state/mnemorph config/mnemorph data cache run; do mkdir -p "$sentinel/$d"; done
chmod 700 "$sentinel" "$sentinel/state" "$sentinel/state/mnemorph" "$sentinel/run"
for f in home/MARKER state/mnemorph/MARKER config/mnemorph/MARKER data/MARKER cache/MARKER run/MARKER; do
  echo "a test must never write here" > "$sentinel/$f"
done

# Every entry under the sentinel: its path, kind, mtime and (for a file) its content hash.
survey() {
  (cd "$sentinel" && find . -print0 | LC_ALL=C sort -z | while IFS= read -r -d '' p; do
    if [ -f "$p" ] && [ ! -L "$p" ]; then
      printf '%s f %s %s\n' "$p" "$(stat -c %Y "$p" 2>/dev/null || stat -f %m "$p")" \
        "$( (sha256sum "$p" 2>/dev/null || shasum -a 256 "$p") | cut -d' ' -f1)"
    else
      printf '%s %s %s\n' "$p" "$( [ -d "$p" ] && echo d || echo o)" \
        "$(stat -c %Y "$p" 2>/dev/null || stat -f %m "$p")"
    fi
  done)
}
before=$(survey)

status=0
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" \
  HOME="$sentinel/home" \
  XDG_STATE_HOME="$sentinel/state" \
  XDG_CONFIG_HOME="$sentinel/config" \
  XDG_DATA_HOME="$sentinel/data" \
  XDG_CACHE_HOME="$sentinel/cache" \
  XDG_RUNTIME_DIR="$sentinel/run" \
  "$@" || status=$?

after=$(survey)
if [ "$before" != "$after" ]; then
  echo "a test wrote to the home it was started under (the sentinel changed):" >&2
  diff <(echo "$before") <(echo "$after") >&2 || true
  exit 1
fi
exit "$status"
