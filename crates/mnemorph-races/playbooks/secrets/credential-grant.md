### credential-grant
Invariant: a grant made for one purpose never reads a credential.
Look: the sandbox's read grants (home, toolchain and package-manager dirs), config reading, the env passed into commands.
Check: for each read root, name the credential paths a toolchain keeps under it; plant a fake credential at one, in a temp HOME or a fixture path, and read it from a sandboxed command. Never read the human's real credential files. A fix denies them inside the grant or narrows it.
Witness: a test reading a planted credential through the grant.
Not a finding: a credential the harness itself reads for an act, never handed to a body.
Regression: reads through toolchain grants: `.cargo/credentials.toml`, `.config/git/credentials`, `.git-credentials`, `.m2/settings.xml`, an `.npmrc` under `.nvm` or home, `.netrc` and auth under `GOPATH`.
