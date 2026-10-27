### detached-work
Invariant: an interrupt stops all the work it is meant to stop: children, background tasks, retries, campaigns.
Look: spawned tasks and processes, tasks without a handle, sleeps inside retries, session end, solo runs, background campaigns.
Check: interrupt mid-retry, mid-tool and mid-commission; list what still runs, spends or writes after.
Witness: a test showing a write, a spend or a process after the interrupt.
Not a finding: finished work flushed to its record.
