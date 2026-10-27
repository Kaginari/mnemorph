### container-image
Invariant: an image is pinned by digest, runs unprivileged, and carries nothing it should not.
Look: `FROM` tags, `USER`, `COPY . .` and `.dockerignore`, secrets in `ARG` or a layer, `curl | sh`, bench task images.
Check: is the base pinned by `@sha256`? Does the context include `.git`, `.env` or keys? Does a secret survive in a layer?
Witness: scanner output (hadolint, a layer listing) saved in the proving ground.
Not a finding: a test-only image never published and never given a secret.
