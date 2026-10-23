# Assets

| Dir | Holds |
|---|---|
| `fonts/` | Cinzel and Rajdhani (SIL Open Font License, the texts beside them), for the cards |
| `portraits/` | one icon per race and ascended form: `<race>.png` (256 px), `<race>@64.png`, `<race>@32.png`, transparent |
| `cards/` | one card per race, ascended form and Veldora, plus the back — drawn by `tools/cards.py` |
| `brand/` | the project's own marks: `banner.jpg` (the README), `social-preview.jpg` (1280×640, GitHub's link card), `mnemorph-icon.png` and its sizes (the emblem: the dragon and the crowned slime, transparent), `mnemorph-hex.png` and its sizes (the emblem in the race icons' hexagon), `favicon.ico` |
| `source/` | the generated images the crops were cut from |

Generated from image-model briefs; the cards' brief is [race-cards-prompt](../docs/race-cards-prompt.md). Missing: the Oni and Arch Dark Elf cards.
The source grids are low resolution (cards ≈ 245 × 395, icons ≈ 250 px): enough for the CLI and
the board's small sizes; the full-size card view waits for higher-resolution renders.

## The race icons

Cut to `portraits/` (256, 64 and 32 px, transparent) from the hexagons of
`source/race-icons.jpg`; Veldora, which that sheet lacks, from `source/icons-grid.png`.

| <img src="portraits/slime@64.png" width="64" alt="Slime"> | <img src="portraits/rimuru@64.png" width="64" alt="Rimuru"> | <img src="portraits/elf@64.png" width="64" alt="Elf"> | <img src="portraits/orc@64.png" width="64" alt="Orc"> | <img src="portraits/kijin@64.png" width="64" alt="Kijin"> | <img src="portraits/dark-elf@64.png" width="64" alt="Dark Elf"> |
|:---:|:---:|:---:|:---:|:---:|:---:|
| Slime | Rimuru | Elf | Orc | Kijin | Dark Elf |

| <img src="portraits/veldora@64.png" width="64" alt="Veldora"> | <img src="portraits/high-elf@64.png" width="64" alt="High Elf"> | <img src="portraits/high-orc@64.png" width="64" alt="High Orc"> | <img src="portraits/named-slime@64.png" width="64" alt="Named Slime"> | <img src="portraits/oni@64.png" width="64" alt="Oni"> | <img src="portraits/arch-dark-elf@64.png" width="64" alt="Arch Dark Elf"> |
|:---:|:---:|:---:|:---:|:---:|:---:|
| Veldora | High Elf | High Orc | Named Slime | Oni | Arch Dark Elf |
