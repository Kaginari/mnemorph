#!/usr/bin/env bash
# nextest's setup script (.config/nextest.toml): a fresh home for the run's tests, handed to every
# test through $NEXTEST_ENV. Nothing of the developer's home is read or written by a test. Under
# tools/test-env.sh, $TMPDIR is that run's own directory, removed when it ends: no home outlives it.
set -euo pipefail
root=$(mktemp -d "${TMPDIR:-/tmp}/mnemorph-test-home.XXXXXX")
for d in home state config data cache run; do mkdir -p "$root/$d"; done
chmod 700 "$root" "$root/state" "$root/run"
{
  echo "HOME=$root/home"
  echo "XDG_STATE_HOME=$root/state"
  echo "XDG_CONFIG_HOME=$root/config"
  echo "XDG_DATA_HOME=$root/data"
  echo "XDG_CACHE_HOME=$root/cache"
  echo "XDG_RUNTIME_DIR=$root/run"
  echo "MNEMORPH_ISOLATED=1"
} >> "${NEXTEST_ENV:?run by nextest as a setup script}"
