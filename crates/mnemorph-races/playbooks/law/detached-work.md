### detached-work
Invariant: an interrupt stops all the work it is meant to stop: every descendant body and command, background tasks, retries, campaigns; and an order it stops is released, never left claimed.
Look: the one `Interrupt` a run shares with the bodies it commissions (`mnemorph-loop` `run.rs`, `commission.rs`); a command's process group (`mnemorph-tools` `shell.rs`); the wake and `settle`, which claim and release queue orders; tasks without a handle; sleeps inside retries; an Arch Dark Elf's background campaign, which must take the same interrupt and budget.
Check: interrupt mid-retry, mid-tool and mid-commission, three bodies down; list what still runs, spends or writes after, and each queue order's state; try a descendant that leaves the group (`setsid`, which the Linux jail refuses).
Witness: a test showing a write, a spend, a process or a claimed order after the interrupt. Held by `esc_stops_every_descendant_and_releases_its_order` and `esc_stops_a_running_command_and_keeps_its_output`.
Not a finding: finished work flushed to its record; a stopped order requeued with its note.
