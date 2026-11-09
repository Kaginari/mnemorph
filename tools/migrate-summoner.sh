#!/bin/sh
# Move a world made before the rename (Rimuru -> Summoner, Veldora -> Player) to the new names:
# the throne's seat, and every creature card that names it. The journal is history and is left as
# it was written.
#
#   sh tools/migrate-summoner.sh [world]
#
# Trust is the human's, so it is never carried over by a script: the cards changed, and the
# witness and the pins still name the old throne. Re-adopt, and re-pin the throne's verify line if
# it has one, with the commands this prints.
set -eu
world=${1:-.}
dir="$world/.mnemorph"
[ -d "$dir" ] || { echo "no .mnemorph in $world" >&2; exit 1; }
if [ -d "$dir/rimuru/rimuru" ]; then
  [ -e "$dir/summoner/summoner" ] && { echo "$dir/summoner/summoner exists already" >&2; exit 1; }
  mkdir -p "$dir/summoner"
  mv "$dir/rimuru/rimuru" "$dir/summoner/summoner"
  rmdir "$dir/rimuru" 2>/dev/null || true
fi
for card in "$dir"/*/*/README.md; do
  [ -f "$card" ] || continue
  perl -pi -e 's/\brimuru\b/summoner/g; s/\bRimuru\b/Summoner/g; s/\bveldora\b/player/g; s/\bVeldora\b/Player/g' "$card"
done
echo "Moved. Now, as the human:"
echo "  mnemorphd roster adopt --world $world --yes"
if grep -q '\*\*Verify:\*\*' "$dir/summoner/summoner/README.md" 2>/dev/null; then
  echo "  mnemorphd verify pin summoner --world $world --yes"
fi
