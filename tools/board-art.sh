#!/usr/bin/env bash
# The race cards the board shows, cut from assets/cards/hires/ into packages/board/public/art/cards/
# as WebP (1200x1680, quality 82), sharp in the full card view, embedded in mnemorphd by the
# board's fixed name map. Run from the repository root after tools/cards.py; needs ImageMagick.
set -euo pipefail
out=packages/board/public/art/cards
mkdir -p "$out"
for race in rimuru elf orc slime kijin dark-elf high-elf high-orc named-slime oni arch-dark-elf; do
    convert "assets/cards/hires/$race.webp" -strip -quality 82 "$out/$race.webp"
done
# The portraits the colony net draws on its canvas: 256 px, in their hexagons.
out=packages/board/public/art/portraits
mkdir -p "$out"
for race in veldora rimuru elf orc slime kijin dark-elf high-elf high-orc named-slime oni arch-dark-elf; do
    convert "assets/portraits/$race@1024.webp" -strip -resize 256x256 -quality 82 "$out/$race.webp"
done
