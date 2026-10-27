### unbounded-delegation
Invariant: commissions and loops end: a depth, a count and a budget bound every chain.
Look: commissioning and `may_dispatch`, routing, the challenge reserve, sequences, re-commission after failure.
Check: can a body commission itself, its parent or a cycle? Is the reserve debited before a body opens? Does a retry open a new body free of charge?
Witness: a test where a chain passes its depth or budget.
Not a finding: a deep chain within its caps.
