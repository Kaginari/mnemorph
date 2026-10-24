//! The Linux side: the jail restricts its own process, then execs the command.

use std::collections::BTreeMap;
use std::path::PathBuf;

use landlock::{
    ABI, Access, AccessFs, CompatLevel, Compatible, LandlockStatus, Ruleset, RulesetAttr,
    RulesetCreatedAttr, RulesetStatus, path_beneath_rules,
};
use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, SeccompRule, TargetArch};

/// The newest landlock ABI the jail asks for; older kernels enforce what they know.
const ABI_ASKED: ABI = ABI::V5;

/// What the jail restricts itself to.
#[derive(Debug, Default)]
pub struct Rules {
    /// Readable and executable.
    pub read: Vec<PathBuf>,
    /// Readable and writable.
    pub write: Vec<PathBuf>,
    /// Sockets allowed.
    pub network: bool,
}

/// Syscalls no sandboxed command may make: debugging or reading other processes, changing
/// mounts or namespaces, loading code into the kernel, the kernel's own keyring, `io_uring`
/// (which would bypass the socket rule).
const ALWAYS_DENIED: &[libc::c_long] = &[
    libc::SYS_ptrace,
    libc::SYS_process_vm_readv,
    libc::SYS_process_vm_writev,
    libc::SYS_mount,
    libc::SYS_umount2,
    libc::SYS_pivot_root,
    libc::SYS_chroot,
    libc::SYS_unshare,
    libc::SYS_setns,
    libc::SYS_keyctl,
    libc::SYS_add_key,
    libc::SYS_request_key,
    libc::SYS_bpf,
    libc::SYS_perf_event_open,
    libc::SYS_kexec_load,
    libc::SYS_kexec_file_load,
    libc::SYS_init_module,
    libc::SYS_finit_module,
    libc::SYS_delete_module,
    libc::SYS_reboot,
    libc::SYS_swapon,
    libc::SYS_swapoff,
    libc::SYS_open_by_handle_at,
    libc::SYS_userfaultfd,
    libc::SYS_io_uring_setup,
    libc::SYS_io_uring_enter,
    libc::SYS_io_uring_register,
    libc::SYS_acct,
    libc::SYS_quotactl,
];

/// Restrict this process: landlock first (it also sets no-new-privileges), then seccomp.
/// Returns the status line for `status`.
///
/// # Errors
///
/// A message naming what could not be enforced; the caller must not run the command then.
pub fn restrict(rules: &Rules) -> Result<String, String> {
    let landlock = landlock(rules)?;
    seccomp(rules.network)?;
    Ok(format!(
        "{landlock} + seccomp, network {}",
        if rules.network { "on" } else { "off" }
    ))
}

fn landlock(rules: &Rules) -> Result<String, String> {
    let fail = |e: &dyn std::fmt::Display| format!("landlock: {e}");
    let status = Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(AccessFs::from_all(ABI_ASKED))
        .map_err(|e| fail(&e))?
        .create()
        .map_err(|e| fail(&e))?
        .add_rules(path_beneath_rules(
            &rules.read,
            AccessFs::from_read(ABI_ASKED),
        ))
        .map_err(|e| fail(&e))?
        .add_rules(path_beneath_rules(
            &rules.write,
            AccessFs::from_all(ABI_ASKED),
        ))
        .map_err(|e| fail(&e))?
        .restrict_self()
        .map_err(|e| fail(&e))?;
    if !status.no_new_privs {
        return Err("no-new-privileges could not be set".into());
    }
    let abi = match status.landlock {
        LandlockStatus::Available {
            effective_abi,
            kernel_abi,
        } => kernel_abi.map_or_else(|| format!("{}", effective_abi as i32), |k| k.to_string()),
        LandlockStatus::NotEnabled => {
            return Err(
                "landlock is built in but not enabled (add it to the lsm= boot parameter)".into(),
            );
        }
        LandlockStatus::NotImplemented => {
            return Err("this kernel has no landlock (5.13 or newer is needed)".into());
        }
    };
    match status.ruleset {
        RulesetStatus::FullyEnforced => Ok(format!("landlock ABI {abi}")),
        RulesetStatus::PartiallyEnforced => Ok(format!(
            "landlock ABI {abi} (of {} asked)",
            ABI_ASKED as i32
        )),
        RulesetStatus::NotEnforced => Err("landlock rules were not enforced".into()),
    }
}

fn seccomp(network: bool) -> Result<(), String> {
    let mut denied: BTreeMap<i64, Vec<SeccompRule>> =
        ALWAYS_DENIED.iter().map(|s| ((*s), Vec::new())).collect();
    if !network {
        // Every socket, Unix ones included: a host daemon's socket (docker, an agent) is a way
        // out. `socketpair` stays: it reaches nothing.
        denied.insert(libc::SYS_socket, Vec::new());
    }
    let arch = TargetArch::try_from(std::env::consts::ARCH).map_err(|e| format!("seccomp: {e}"))?;
    let filter = SeccompFilter::new(
        denied,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        arch,
    )
    .map_err(|e| format!("seccomp: {e}"))?;
    let program: BpfProgram = filter
        .try_into()
        .map_err(|e: seccompiler::BackendError| format!("seccomp: {e}"))?;
    seccompiler::apply_filter(&program).map_err(|e| format!("seccomp: {e}"))
}
