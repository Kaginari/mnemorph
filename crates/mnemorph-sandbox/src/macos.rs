//! The macOS side: a Seatbelt profile for `sandbox-exec`.

use std::fmt::Write as _;

use crate::Policy;

/// The profile for `policy`: deny by default; read everywhere but the hidden paths; write only
/// the policy's paths; the network only when allowed.
pub(crate) fn profile(policy: &Policy) -> String {
    let mut p = String::from(
        "(version 1)\n\
         (deny default)\n\
         (allow process-exec)\n\
         (allow process-fork)\n\
         (allow signal (target same-sandbox))\n\
         (allow process-info* (target same-sandbox))\n\
         (allow sysctl-read)\n\
         (allow mach-lookup)\n\
         (allow ipc-posix-shm*)\n\
         (allow ipc-posix-sem*)\n\
         (allow iokit-open)\n\
         (allow file-ioctl)\n\
         (allow pseudo-tty)\n\
         (allow file-read*)\n",
    );
    for path in Policy::existing(&policy.hidden)
        .iter()
        .chain(policy.hidden.iter())
    {
        let _ = writeln!(
            p,
            "(deny file-read* (subpath {}))",
            quote(&path.to_string_lossy())
        );
    }
    p.push_str("(allow file-write*");
    for path in Policy::existing(&policy.write) {
        let _ = write!(p, " (subpath {})", quote(&path.to_string_lossy()));
    }
    p.push_str(")\n");
    if policy.network {
        p.push_str("(allow network*)\n(allow system-socket)\n");
    }
    p
}

/// A Seatbelt string literal.
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
