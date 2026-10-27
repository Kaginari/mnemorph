### approval-binding
Invariant: the act that runs is the card the human approved: same verb, target, arguments and bytes, once.
Look: the gate's card, the approval record, the act runner, files an act reads when it runs (a PR body, a message file), retries and resumes, headless and non-TTY approval.
Check: compare the approved object (argv, target, file hashes) with what executes; change a referenced file after approval; replay an approval on a retry; type before the card shows.
Witness: a test approving card A and observing act B, or one approval running twice.
Not a finding: an act the human approved and meant, however unwise.
Regression: a file swapped after approval (the PR body file): hash it at approval, run the frozen bytes. Headless approval typeahead: input buffered before the card renders never answers it.
