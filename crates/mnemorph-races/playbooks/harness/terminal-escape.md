### terminal-escape
Invariant: model, tool and repository text reaches the terminal as inert characters.
Look: approval cards, status lines, the transcript, diffs, file and branch names, errors; anything printed raw.
Check: put ESC, CSI, OSC (`]52` clipboard, `]8` links), `\r` overwrites, C1 bytes and bidi overrides in each field; is it stripped or escaped at the render boundary, before layout?
Witness: a test rendering a crafted field and asserting the escape reaches the output, or the card shows other text than it holds.
Not a finding: escapes in text the human typed; styling the harness emits itself.
Regression: escape injection into cards and status; every model-supplied string is sanitised where it is rendered, not where it came from.
