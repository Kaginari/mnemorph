### tool-argument-injection
Invariant: each tool's handler validates its arguments; the schema shapes them, it does not authorise them.
Look: argument decoding of every tool (bash, edit, write, commission, verify, coverage, outward, queue), defaults, aliases, extra fields, duplicate keys, paths and targets among the fields.
Check: compare the schema with the handler; send an extra field, a duplicate key, a coerced type, an out-of-range value, a path or target outside the body's territory.
Witness: a test calling the tool with the crafted arguments and showing an effect the shelf or territory forbids.
Not a finding: a malformed call refused with an error.
