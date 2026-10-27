### request-forgery
Invariant: the server fetches only what it was configured to, never a URL a request names.
Look: base URLs, webhook or callback URLs taken from requests, fetch tools, redirects followed, proxies.
Check: can a client or a body set a URL the server fetches? Are loopback, link-local (`169.254.169.254`) and private ranges refused after redirects and DNS?
Witness: a loopback test where the server fetches a planted local listener.
Not a finding: a URL in the human's own config.
