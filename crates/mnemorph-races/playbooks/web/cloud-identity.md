### cloud-identity
Invariant: a deployed Mnemorph's identity (service account, token, key) grants only what it uses.
Look: deployment examples, the daemon's config and environment credentials, metadata-endpoint reach, container capabilities.
Check: list each credential the daemon holds and what it permits; can a body reach the metadata endpoint or the daemon's environment?
Witness: a loopback proof or a test over the config showing the overreach.
Not a finding: a broad credential in a local single-user example marked as such.
