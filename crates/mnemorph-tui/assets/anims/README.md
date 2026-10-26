# Race portraits

Each race's idle loop at 24 pixels square, in the `.mnanim.json` format (v1) of
[Kaginari/mnemorph-animations](https://github.com/Kaginari/mnemorph-animations), which draws
them from the race portraits: its `anims/24px/<race>.mnanim.json` (`kind: "idle"`, 16 fps, a
seamless loop), copied here as `<race>-24.mnanim.json`. They are embedded in the binary and
decoded once by `src/kit/mnanim.rs`; the sidebar plays the working body's loop at its head, and
the welcome card shows Rimuru's first frame.

Until the real files land here, `placeholder-24.mnanim.json` stands in for every race: a
hand-made slime, eight frames of a bounce, whose body, light and shadow (palette entries 1, 2
and 4) are tinted toward the race's colour. A `<race>-24` file added here, and named in
`kit::mnanim::FILES`, takes over from the placeholder for that race.
