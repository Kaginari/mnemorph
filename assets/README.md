# Assets

| Dir | Holds |
|---|---|
| `portraits/` | one icon per race and ascended form: `<race>.png` (256 px), `<race>@64.png`, `<race>@32.png`, transparent |
| `cards/` | card art per race, cropped to the frame, transparent corners; `back.png` is the card back |
| `source/` | the generated grids the crops were cut from |

Generated from the briefs in [race-icons-prompt](../docs/race-icons-prompt.md) and
[race-cards-prompt](../docs/race-cards-prompt.md). Missing: the Oni and Arch Dark Elf cards.
The source grids are low resolution (cards ≈ 245 × 395, icons ≈ 250 px): enough for the CLI and
the board's small sizes; the full-size card view waits for higher-resolution renders.
