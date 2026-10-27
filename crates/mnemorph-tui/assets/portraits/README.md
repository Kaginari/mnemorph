# Veldora's portrait, in pixels

Veldora's portrait — the human, whom the sidebar shows while no body works — a 256 × 256 RGBA PNG
from [Kaginari/mnemorph-animations](https://github.com/Kaginari/mnemorph-animations)'
`assets/portraits/` at commit `df20cfa`. It is embedded in the binary by `src/kit/pixels.rs` and
drawn as real pixels — Kitty, iTerm2's inline images or Sixel — where the terminal draws images,
resized once with Lanczos to its box; elsewhere in half blocks. The races' own portraits are their
animations, in `../anims/256px/`.
