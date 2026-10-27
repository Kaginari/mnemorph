### frame-confusion
Invariant: every stream and message has one reading; producer and consumer agree on its frames.
Look: provider SSE parsing, tool-call deltas, JSON-lines journals, length-prefixed or stdio framing, truncation at caps.
Check: split a frame across reads; put a newline or `data:` inside a value; cut mid-UTF-8 or mid-event; repeat a tool-call id; send an unknown event type.
Witness: a test with a recorded cassette showing one frame read as two, or a truncated frame accepted as whole.
Not a finding: a malformed stream refused with an error.
