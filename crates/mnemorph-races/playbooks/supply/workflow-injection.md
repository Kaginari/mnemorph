### workflow-injection
Invariant: untrusted event data never reaches a CI step's script, and each job's token holds only what it needs.
Look: `${{ github.event.* }}` in `run:`, `pull_request_target`, `workflow_run`, a PR head checked out in a privileged job, `permissions:`, actions not pinned by SHA, caches and artifacts shared across trust.
Check: list the fields an outsider writes (titles, branch names, bodies) and trace each into a shell; read each job's token permissions.
Witness: actionlint or zizmor output saved as scanner output, naming the step.
Not a finding: event data passed through `env:` and quoted; a write token on a job only maintainers trigger.
