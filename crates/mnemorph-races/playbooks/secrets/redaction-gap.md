### redaction-gap
Invariant: a secret never reaches a journal, log, transcript, card or report unredacted.
Look: the redactor and every write that skips it: journals, beats, `log.md`, errors, echoed tool output, provider request logs, the board's stream.
Check: plant a key in env, config and a tool's output; grep every record written after; try it split across chunks, base64 and URL-encoded.
Witness: a test showing the planted secret in a written record.
Not a finding: a redacted prefix kept to identify the key, by design.
