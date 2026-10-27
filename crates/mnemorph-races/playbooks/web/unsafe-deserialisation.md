### unsafe-deserialisation
Invariant: untrusted bytes deserialise into bounded, typed data and nothing more.
Look: serde on request bodies and protocol messages, records read back, `deny_unknown_fields`, untagged enums, size and depth limits, the protocol's generated TypeScript against the Rust.
Check: send oversize, deeply nested, duplicate-key, unknown-field and type-confused payloads; compare how Rust and TypeScript read one message.
Witness: a test showing an unbounded allocation, a crash, or two readings of one message.
Not a finding: a parse error returned cleanly.
