//! The sandbox: landlock and seccomp on Linux, Seatbelt on macOS, the network policy.
//!
//! A command runs inside a [`Policy`]: it may write only the world and its own temporary
//! directory, read only the system's directories, the world and the toolchains in the home —
//! never a credential — and reach the network only when the policy says so.
//!
//! - **Linux**: the command is started through `mnemorph-guard`, which restricts itself with
//!   landlock (the file rules) and seccomp (no sockets without network; no `ptrace`, `mount`,
//!   `unshare`, `bpf`, `io_uring`, … ever), then execs the command. No namespaces are needed, so
//!   it works where unprivileged user namespaces are restricted.
//! - **macOS**: the command is started through `sandbox-exec` with a generated Seatbelt profile.
//!
//! [`Sandbox::status`] says what is actually in force, or `none — <reason>`: containment is
//! never claimed.

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
mod macos;

/// Credential paths under the home: each covers itself and all beneath it. No sandbox rule names
/// them or a folder above them, Seatbelt hides them as well, and the shell classifier refuses
/// commands that name them. Landlock cannot hide a path inside a tree it reads, so the toolchain
/// trees that hold one — `.cargo`, `.cache` — are granted part by part ([`HOME_TOOLCHAINS`]).
pub const HOME_CREDENTIALS: &[&str] = &[
    ".ssh",
    ".aws",
    ".gnupg",
    ".password-store",
    ".netrc",
    ".git-credentials",
    ".git-credential-cache",
    ".npmrc",
    ".pypirc",
    ".docker",
    ".kube",
    ".config/gh",
    ".config/gcloud",
    ".azure",
    ".cargo/credentials",
    ".cargo/credentials.toml",
    "Library/Keychains",
    // uv's `uv auth` logins, beside the interpreters and tools the toolchains name.
    ".local/share/uv/credentials",
    // Hugging Face's login tokens — its model cache, `hub`, stays readable — and git's
    // credential-cache socket.
    ".cache/huggingface/token",
    ".cache/huggingface/stored_tokens",
    ".cache/git",
];

/// Credential files anywhere on the system.
pub const SYSTEM_CREDENTIALS: &[&str] = &["/etc/shadow", "/etc/gshadow", "/etc/sudoers"];

/// Toolchains and caches in the home that builds read, on every platform. A tree that holds a
/// credential is named part by part — Landlock cannot hide a path inside a tree it reads — so
/// only what builds use is named, and a cache missing here is unreadable rather than a hole.
pub const HOME_TOOLCHAINS: &[&str] = &[
    // Not `.cargo` whole: `credentials.toml` holds the crates.io tokens. Cargo reads its
    // binaries, the registry's index and sources, git dependencies and its config; its locks and
    // usage database (`.package-cache`, `.global-cache`) are written, which a read grant does
    // not give anyway, and cargo goes on without them.
    ".cargo/bin",
    ".cargo/registry",
    ".cargo/git",
    ".cargo/config.toml",
    ".cargo/config",
    ".cargo/env",
    ".rustup",
    // `.local` only in part: `.local/share/keyrings` holds credentials.
    ".local/bin",
    ".local/lib",
    ".local/share/pnpm",
    ".local/go-current",
    ".npm",
    ".nvm",
    ".pnpm-store",
    // Not `.cache` whole: `huggingface/token` and git's credential cache live there. The named
    // caches of Python (uv, pip, Poetry, pre-commit), Go, Node (corepack under `node`, pnpm,
    // node-gyp, TypeScript, Yarn, Playwright's browsers), Deno, Bazel, sccache, Hugging Face's
    // models, and the Kaginari bench's interpreters.
    ".cache/uv",
    ".cache/pip",
    ".cache/pypoetry",
    ".cache/pre-commit",
    ".cache/go-build",
    ".cache/golangci-lint",
    ".cache/node",
    ".cache/pnpm",
    ".cache/node-gyp",
    ".cache/typescript",
    ".cache/yarn",
    ".cache/ms-playwright",
    ".cache/deno",
    ".cache/bazel",
    ".cache/sccache",
    ".cache/huggingface/hub",
    ".cache/kaginari-bench",
    ".pyenv",
    // uv's interpreters and installed tools — the targets behind `.local/bin`'s links — but not
    // `.local/share/uv` whole: its `credentials/` holds the logins `uv auth` keeps.
    ".local/share/uv/python",
    ".local/share/uv/tools",
    ".bun",
    ".deno",
    ".gitconfig",
    ".config/git",
    "go",
    ".gradle",
    ".m2",
];

/// The system directories a command may read (and execute from).
const SYSTEM_READ: &[&str] = &[
    "/usr",
    "/bin",
    "/sbin",
    "/lib",
    "/lib32",
    "/lib64",
    "/libx32",
    "/etc",
    "/opt",
    "/proc",
    "/sys",
    "/dev",
    "/nix",
    "/snap",
    "/System",
    "/Library",
    "/Applications",
    "/private/etc",
    "/private/var/db",
    "/var/db",
    "/var/select",
];

/// Device files a command may write.
const DEVICES: &[&str] = &[
    "/dev/null",
    "/dev/zero",
    "/dev/full",
    "/dev/tty",
    "/dev/random",
    "/dev/urandom",
];

/// What a sandboxed command may touch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// Read and written: the world and the command's temporary directory.
    pub write: Vec<PathBuf>,
    /// Read (and executed from) only.
    pub read: Vec<PathBuf>,
    /// Paths never readable even under a readable directory (applied where the platform can
    /// deny inside an allowed tree, Seatbelt; on Linux they are simply never allowed).
    pub hidden: Vec<PathBuf>,
    /// Sockets allowed.
    pub network: bool,
}

impl Policy {
    /// The policy for a command in `world` with `home`, writing temporary files under `tmp`.
    #[must_use]
    pub fn new(world: &Path, home: &Path, tmp: &Path, network: bool) -> Self {
        let mut read: Vec<PathBuf> = SYSTEM_READ.iter().map(PathBuf::from).collect();
        read.extend(HOME_TOOLCHAINS.iter().map(|t| home.join(t)));
        let mut write = vec![world.to_path_buf(), tmp.to_path_buf()];
        write.extend(DEVICES.iter().map(PathBuf::from));
        let mut hidden: Vec<PathBuf> = HOME_CREDENTIALS.iter().map(|c| home.join(c)).collect();
        hidden.extend(SYSTEM_CREDENTIALS.iter().map(PathBuf::from));
        Self {
            write,
            read,
            hidden,
            network,
        }
    }

    /// The paths of `list` that exist, resolved; rules cannot name what is not there.
    #[must_use]
    pub fn existing(list: &[PathBuf]) -> Vec<PathBuf> {
        list.iter().filter_map(|p| p.canonicalize().ok()).collect()
    }
}

/// How commands are contained on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sandbox {
    /// Through the `mnemorph-guard` helper at this path (Linux).
    Guard(PathBuf),
    /// Through `sandbox-exec` (macOS).
    Seatbelt(PathBuf),
    /// Not at all, and why.
    None(String),
}

impl Sandbox {
    /// Find the sandbox for this machine: `MNEMORPH_GUARD`, or `mnemorph-guard` beside the running
    /// binary, on Linux; `/usr/bin/sandbox-exec` on macOS.
    #[must_use]
    pub fn detect() -> Self {
        if cfg!(target_os = "linux") {
            let candidate = std::env::var_os("MNEMORPH_GUARD")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::current_exe()
                        .ok()
                        .and_then(|exe| exe.parent().map(|d| d.join("mnemorph-guard")))
                });
            match candidate {
                Some(p) if p.is_file() => Self::Guard(p),
                Some(p) => Self::None(format!("the guard helper is missing ({})", p.display())),
                None => Self::None("the guard helper cannot be located".into()),
            }
        } else if cfg!(target_os = "macos") {
            let p = PathBuf::from("/usr/bin/sandbox-exec");
            if p.is_file() {
                Self::Seatbelt(p)
            } else {
                Self::None("sandbox-exec is missing".into())
            }
        } else {
            Self::None(format!("no sandbox for {}", std::env::consts::OS))
        }
    }

    /// A command running `program` with `args` inside `policy`.
    #[must_use]
    pub fn command(&self, policy: &Policy, program: &str, args: &[&str]) -> Command {
        match self {
            Self::Guard(guard) => {
                let mut cmd = Command::new(guard);
                let read = Policy::existing(&policy.read);
                let write = Policy::existing(&policy.write);
                // Past a few hundred rules the command line would not hold them: they go in a
                // private file the guard reads, then removes, before it restricts itself.
                if let Some(file) = (read.len() + write.len() > RULES_ON_ARGV)
                    .then(|| rules_file(&read, &write))
                    .flatten()
                {
                    cmd.arg("--rules").arg(file);
                } else {
                    for p in read {
                        cmd.arg("--read").arg(p);
                    }
                    for p in write {
                        cmd.arg("--write").arg(p);
                    }
                }
                if policy.network {
                    cmd.arg("--network");
                }
                cmd.arg("--").arg(program).args(args);
                cmd
            }
            #[cfg(target_os = "macos")]
            Self::Seatbelt(exec) => {
                let mut cmd = Command::new(exec);
                cmd.arg("-p")
                    .arg(macos::profile(policy))
                    .arg(program)
                    .args(args);
                cmd
            }
            #[cfg(not(target_os = "macos"))]
            Self::Seatbelt(_) => {
                let mut cmd = Command::new(program);
                cmd.args(args);
                cmd
            }
            Self::None(_) => {
                let mut cmd = Command::new(program);
                cmd.args(args);
                cmd
            }
        }
    }

    /// What is in force, in one line — `landlock ABI 1 + seccomp, network off`, or
    /// `none — <reason>`. On Linux the guard restricts a probe and reports what the kernel did.
    #[must_use]
    pub fn status(&self, network: bool) -> String {
        match self {
            Self::Guard(guard) => {
                let mut probe = Command::new(guard);
                probe.arg("--probe");
                if network {
                    probe.arg("--network");
                }
                match probe.output() {
                    Ok(out) if out.status.success() => {
                        String::from_utf8_lossy(&out.stdout).trim().to_owned()
                    }
                    Ok(out) => format!(
                        "none — the guard refused: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    ),
                    Err(e) => format!("none — the guard did not start: {e}"),
                }
            }
            Self::Seatbelt(_) => {
                format!("seatbelt, network {}", if network { "on" } else { "off" })
            }
            Self::None(reason) => format!("none — {reason}"),
        }
    }
}

/// Make this process unreadable to the commands it starts: on Linux it becomes non-dumpable,
/// so `/proc/<pid>/environ` and its memory are closed to every non-root process.
///
/// # Errors
///
/// When the kernel refuses.
pub fn harden_self() -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)?;
    }
    Ok(())
}

/// The most rules passed on the guard's command line; past it they go in a file.
const RULES_ON_ARGV: usize = 256;

/// Write the rules to a fresh owner-only file outside every world: `r` or `w`, then a path,
/// each NUL-terminated. `None` when no such file could be made (the rules then go on argv).
fn rules_file(read: &[PathBuf], write: &[PathBuf]) -> Option<PathBuf> {
    use std::io::Write as _;
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let path =
        std::env::temp_dir().join(format!("mnemorph-rules-{}-{stamp:x}", std::process::id()));
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .ok()?;
    let mut bytes = Vec::new();
    for (kind, list) in [(b'r', read), (b'w', write)] {
        for p in list {
            bytes.extend([kind, 0]);
            bytes.extend(p.as_os_str().as_bytes());
            bytes.push(0);
        }
    }
    f.write_all(&bytes).ok()?;
    Some(path)
}
