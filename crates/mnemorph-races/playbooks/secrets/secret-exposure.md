### secret-exposure
Invariant: no live secret lives in the tree or its history.
Look: the diff and history: keys and tokens (`-----BEGIN`, `sk-`, `ghp_`, `AKIA`), `.env`, fixtures, cassettes and journals committed by mistake, `.gitignore`.
Check: run a secret scanner over the tree and `git log -p`; for each hit decide whether it is live (format, entropy, a fixture marker).
Witness: the scanner output naming file and commit, redacted, saved in the proving ground.
Not a finding: an obvious test value (`sk-test-`, zeros); a revoked key documented as revoked.
