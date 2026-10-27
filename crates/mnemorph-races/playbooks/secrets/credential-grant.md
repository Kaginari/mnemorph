### credential-grant
Invariant: a grant made for one purpose never reads a credential.
Look: the sandbox's read grants (home, toolchain and package-manager dirs), config reading, the env passed into commands.
Check: for each read root, list the credential files under it and read them from a sandboxed command; a fix denies them inside the grant or narrows it.
Witness: a test reading a planted credential through the grant.
Not a finding: a credential the harness itself reads for an act, never handed to a body.
Regression: reads through toolchain grants: `.cargo/credentials.toml`, `.config/git/credentials`, `.git-credentials`, `.m2/settings.xml`, an `.npmrc` under `.nvm` or home, `.netrc` and auth under `GOPATH`.
