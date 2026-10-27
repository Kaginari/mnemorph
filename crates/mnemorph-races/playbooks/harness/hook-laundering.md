### hook-laundering
Invariant: an outward act does only what its classifier read; git config, hooks and the environment add no hidden act.
Look: the act runner's and outward tool's git and gh calls, the classifier's parse of git, env prefixes, repository config and includes, hooks, aliases.
Check: does `-c core.sshCommand=`, a `GIT_SSH_COMMAND=` or `GIT_CONFIG_*` prefix, `remote.*.pushurl`, `remote.*.mirror`, `push.followTags`, `core.hooksPath` or a short refspec (`:main`, `+x`) change what runs? The runner pins config, URL and full refspecs.
Witness: a test where planted config or a prefix sends the approved push elsewhere, deletes or forces a ref, pushes tags, or runs a command.
Not a finding: the human's own global config, which no body can write.
Regression: `-c`, `GIT_SSH_COMMAND`, `pushurl`, mirror, followTags, short refspecs.
