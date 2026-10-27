### auth-bypass
Invariant: every request that reads or changes a session is authenticated and authorised for that session.
Look: the daemon's and server's routes, middleware order, the loopback token or socket, WebSocket upgrades, the SDK's client.
Check: list each route with its guard; call it with no token, another session's id, and an alternate path (WebSocket instead of HTTP, a batch).
Witness: a proof-of-concept test on loopback showing the request served without its auth.
Not a finding: a public health route that reveals nothing.
