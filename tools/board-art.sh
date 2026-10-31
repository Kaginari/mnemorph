#!/usr/bin/env bash
# The race cards the board shows, cut from assets/cards/ into packages/board/public/art/cards/ as
# WebP (600x840, quality 82): about 40 KB a card instead of 300 KB, embedded in mnemorphd by the
# board's fixed name map. Run from the repository root after tools/cards.py; needs ImageMagick.
set -euo pipefail
out=packages/board/public/art/cards
mkdir -p "$out"
for race in rimuru elf orc slime kijin dark-elf high-elf high-orc named-slime oni arch-dark-elf; do
    convert "assets/cards/$race.png" -strip -quality 82 "$out/$race.webp"
done
