### mcp-identity
Invariant: a tool call and its result are bound to the configured server and the outstanding request, never to a name a peer chose.
Look: tool server registration, naming and namespaces, request ids, reconnects, descriptions and schemas a peer sends, any path that calls a tool outside the conduit.
Check: two servers claim one tool name; a result answers another server's request id; a description claims authority; a reconnect rebinds a name.
Witness: a test with two fake servers where one's tool shadows, or its result satisfies, the other's.
Not a finding: a peer description that only steers the model while the handler still checks.
