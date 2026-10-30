//! The macOS side: a Seatbelt profile for `sandbox-exec`.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::Policy;

/// The system services a command may reach: name lookups, logging, notifications, preferences,
/// certificate trust. Not `LaunchServices`: through it a command could start an application
/// outside the sandbox (`open -a Terminal`).
const SERVICES: &[&str] = &[
    "com.apple.system.opendirectoryd.libinfo",
    "com.apple.system.opendirectoryd.membership",
    "com.apple.system.notification_center",
    "com.apple.system.logger",
    "com.apple.logd",
    "com.apple.diagnosticd",
    "com.apple.cfprefsd.daemon",
    "com.apple.cfprefsd.agent",
    "com.apple.trustd",
    "com.apple.trustd.agent",
    "com.apple.SecurityServer",
    "com.apple.bsd.dirhelper",
    "com.apple.FSEvents",
];

/// The services name resolution needs, when the network is open.
const NETWORK_SERVICES: &[&str] = &[
    "com.apple.dnssd.service",
    "com.apple.SystemConfiguration.configd",
];

/// The profile for `policy`: deny by default; read only the policy's paths (metadata anywhere,
/// for path lookups), never the hidden ones; write only the policy's paths; the network and
/// its services only when allowed.
pub(crate) fn profile(policy: &Policy) -> String {
    let mut p = String::from(
        "(version 1)\n\
         (deny default)\n\
         (allow process-exec)\n\
         (allow process-fork)\n\
         (allow signal (target same-sandbox))\n\
         (allow process-info* (target same-sandbox))\n\
         (allow sysctl-read)\n\
         (allow ipc-posix-shm*)\n\
         (allow ipc-posix-sem*)\n\
         (allow file-ioctl)\n\
         (allow pseudo-tty)\n\
         (allow file-read-metadata)\n",
    );
    // Another process's arguments and environment (`kern.procargs2`, `kern.proc.*`) stay shut,
    // so the harness's key is not read through sysctl. Seatbelt applies the last rule that
    // matches, so this deny follows the allow above. Not yet verified on a Mac beyond CI.
    p.push_str(
        "(deny sysctl-read (sysctl-name \"kern.procargs2\") (sysctl-name \"kern.procargs\") \
         (sysctl-name-prefix \"kern.proc.\"))\n",
    );
    let readable: Vec<PathBuf> = Policy::existing(&policy.read)
        .into_iter()
        .chain(Policy::existing(&policy.write))
        .collect();
    p.push_str("(allow file-read* (literal \"/\")");
    for path in &readable {
        let _ = write!(p, " (subpath {})", quote(&path.to_string_lossy()));
    }
    p.push_str(")\n");
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
    p.push_str(")\n(allow mach-lookup");
    let services = SERVICES.iter().chain(if policy.network {
        NETWORK_SERVICES
    } else {
        &[]
    });
    for name in services {
        let _ = write!(p, " (global-name {})", quote(name));
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
