# Race animations

The races' animations drawn natively at 256 pixels square, in the `.mnanim.json` format (v2) of
[Kaginari/mnemorph-animations](https://github.com/Kaginari/mnemorph-animations): each
`<name>.mnanim.json` holds the header and palette, and its `<name>.mnanim.png` the frames as an
8-bit indexed sprite sheet.

- `256px/<race>.mnanim.json`: the eleven idle loops, `kind: "idle"`, 48 frames at 16 fps, seamless.
- `256px/ascend-<base>.mnanim.json`: the five ascensions, `kind: "ascend"`, 80 frames at 16 fps,
  played once; `race` names the risen form.

They are embedded in the binary (7.5 MB the sixteen) and read by `src/kit/mnanim.rs`. The sidebar
plays the working body's clip at its head: as real pixels where the terminal draws images, else as
glyph cells.

Source: mnemorph-animations at commit `ff8af6d`, `anims/256px/` (generated there with
`python3 gen/mnanim.py --canvas 256`). To refresh them, copy that directory's 32 files here.
