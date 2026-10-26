# Race portraits, in pixels

Each race's portrait, 256 × 256 RGBA PNG, from
[Kaginari/mnemorph-animations](https://github.com/Kaginari/mnemorph-animations)'
`assets/portraits/` at commit `df20cfa` (1.2 MB for the eleven). They are embedded in the binary
by `src/kit/pixels.rs` and drawn as real pixels — Kitty, iTerm2's inline images or Sixel — where
the terminal draws images, each resized once with Lanczos to its box; elsewhere the half-block
loops in `../anims/` are drawn. To refresh them, copy the eleven files from a newer commit here.
