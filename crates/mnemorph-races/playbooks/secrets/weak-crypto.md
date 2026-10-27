### weak-crypto
Invariant: security values use sound primitives: CSPRNG tokens, SHA-256 or better for integrity, constant-time comparison of secrets.
Look: token and nonce generation, hashes standing for identity (fingerprints, FNV keys), `==` on tokens, key files and their modes, TLS settings.
Check: is a non-cryptographic hash used where an attacker picks the input and a collision matters? Is a secret compared in variable time? Is a token drawn from a seeded RNG?
Witness: a test producing the collision or showing the comparison, or scanner output.
Not a finding: FNV naming a file where a collision costs only a cache miss.
