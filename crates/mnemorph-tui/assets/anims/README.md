# Race portraits

Each race's idle loop at 32 pixels square (32 columns × 16 rows in half blocks), in the
`.mnanim.json` format (v1) of
[Kaginari/mnemorph-animations](https://github.com/Kaginari/mnemorph-animations): `kind: "idle"`,
48 frames at 16 fps, a seamless loop. They are embedded in the binary and decoded once per race
by `src/kit/mnanim.rs`; the sidebar plays the working body's loop at its head, and the welcome
card shows Rimuru's first frame.

Source: mnemorph-animations at commit `df20cfa`, generated with `python3 gen/mnanim.py --size 32`
(one file per race, `<race>.mnanim.json`; 1.1 MB for the eleven). To refresh them, run that
command at a newer commit and copy the eleven files here.
