### resource-exhaustion
Invariant: every unit of work has a bound: tokens, time, memory, disk, retries.
Look: envelopes and caps, the provider's retry, buffers read whole, records grown without a cap, regexes over untrusted text, recursion.
Check: find each loop's counter and where it is checked; is the cap enforced before the spend? Does a count reset on resume?
Witness: a test showing a cap passed, or work growing with input past its bound.
Not a finding: a cost the human configured; slowness inside every cap.
