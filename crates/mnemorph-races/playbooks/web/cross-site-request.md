### cross-site-request
Invariant: a page on another origin cannot drive the local server.
Look: CORS headers, Origin and Host checks (DNS rebinding), WebSocket origin, cookies and SameSite, simple-request content types.
Check: send a foreign Origin, a rebinding Host, a `text/plain` POST, a cross-site WebSocket upgrade.
Witness: a loopback test where the foreign-origin request succeeds.
Not a finding: an uncredentialed read of nothing private.
