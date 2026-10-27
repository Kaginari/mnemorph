### data-isolation
Invariant: a world, session or user reads only its own records.
Look: state under `$XDG_STATE_HOME/mnemorph/`, keys like `<world>-<hash>`, journals, challenge records, caches, the server's session stores, file modes.
Check: can one world's key collide with another's? Are folders `0700` and files `0600`? Does a session id sent by a client select another's data?
Witness: a test with two worlds or sessions where one reads the other's record.
Not a finding: a shared read the human configured.
