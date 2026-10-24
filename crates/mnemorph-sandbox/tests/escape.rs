//! The escape suite (M4's exit): writes outside the world, network from a body without it,
//! symlink and `..` tricks, `/proc` reads of the harness's secrets, credential paths — every
//! one refused, on Linux (the real guard) and on macOS (the real `sandbox-exec`).

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;

use mnemorph_sandbox::{Policy, Sandbox};

struct Ground {
    root: PathBuf,
}

impl Ground {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mnemorph-escape-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for d in [
            "world",
            "outside",
            "home/.ssh",
            "home/.aws",
            "home/Documents",
            "tmp",
        ] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        fs::write(root.join("home/.ssh/id_ed25519"), "PRIVATE KEY\n").unwrap();
        fs::write(
            root.join("home/.aws/credentials"),
            "aws_secret_access_key=x\n",
        )
        .unwrap();
        fs::write(root.join("outside/target.txt"), "untouched\n").unwrap();
        fs::write(root.join("home/Documents/diary.txt"), "dear diary\n").unwrap();
        let root = root.canonicalize().unwrap();
        Self { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn policy(&self, network: bool) -> Policy {
        Policy::new(
            &self.path("world"),
            &self.path("home"),
            &self.path("tmp"),
            network,
        )
    }

    /// Run `script` with bash inside `policy`, from the world's root.
    fn run(&self, policy: &Policy, script: &str) -> (bool, String) {
        let out = sandbox()
            .command(
                policy,
                "/bin/bash",
                &["--noprofile", "--norc", "-c", script],
            )
            .current_dir(self.path("world"))
            .env("HOME", self.path("home"))
            .env("TMPDIR", self.path("tmp"))
            .output()
            .expect("the sandbox starts");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        (out.status.success(), text)
    }
}

impl Drop for Ground {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn sandbox() -> Sandbox {
    if cfg!(target_os = "linux") {
        Sandbox::Guard(PathBuf::from(env!("CARGO_BIN_EXE_mnemorph-guard")))
    } else {
        Sandbox::detect()
    }
}

#[test]
fn the_sandbox_is_in_force_and_says_so() {
    let s = sandbox();
    let off = s.status(false);
    let on = s.status(true);
    assert!(!off.starts_with("none"), "{off}");
    assert!(off.ends_with("network off"), "{off}");
    assert!(on.ends_with("network on"), "{on}");
    if cfg!(target_os = "linux") {
        assert!(
            off.starts_with("landlock ABI") && off.contains("seccomp"),
            "{off}"
        );
    }
}

#[test]
fn the_world_stays_writable() {
    let g = Ground::new("inside");
    let (ok, out) = g.run(
        &g.policy(false),
        "echo hi > notes.txt && mkdir -p a/b && mv notes.txt a/b/ && cat a/b/notes.txt",
    );
    assert!(ok, "{out}");
    assert_eq!(out, "hi\n");
}

#[test]
fn writes_outside_the_world_are_refused() {
    let g = Ground::new("outside");
    let target = g.path("outside/target.txt");
    for script in [
        format!("echo pwned > {}", target.display()),
        "echo pwned > ../outside/target.txt".to_owned(),
        "cd .. && echo pwned > outside/target.txt".to_owned(),
        format!("cp /etc/hosts {}", g.path("outside/new.txt").display()),
        format!("rm {}", target.display()),
    ] {
        let (ok, out) = g.run(&g.policy(false), &script);
        assert!(!ok, "{script} succeeded: {out}");
    }
    assert_eq!(fs::read_to_string(&target).unwrap(), "untouched\n");
    assert!(!g.path("outside/new.txt").exists());
}

#[test]
fn a_symlink_out_of_the_world_leads_nowhere() {
    let g = Ground::new("symlink");
    std::os::unix::fs::symlink(g.path("outside"), g.path("world/link")).unwrap();
    let (ok, out) = g.run(&g.policy(false), "echo pwned > link/target.txt");
    assert!(!ok, "{out}");
    let (ok, out) = g.run(
        &g.policy(false),
        "ln -sf ../outside/target.txt l2 && echo pwned > l2",
    );
    assert!(!ok, "{out}");
    assert_eq!(
        fs::read_to_string(g.path("outside/target.txt")).unwrap(),
        "untouched\n"
    );
}

#[test]
fn credentials_cannot_be_read() {
    let g = Ground::new("credentials");
    for script in [
        "cat ~/.ssh/id_ed25519",
        "cat \"$HOME/.aws/credentials\"",
        "ls ~/.ssh",
    ] {
        let (ok, out) = g.run(&g.policy(false), script);
        assert!(
            !ok && !out.contains("PRIVATE KEY") && !out.contains("aws_secret"),
            "{script}: {out}"
        );
    }
}

#[test]
fn the_network_is_closed_unless_the_policy_opens_it() {
    let g = Ground::new("network");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let probe = format!("exec 3<>/dev/tcp/127.0.0.1/{port} && echo connected");
    let (ok, out) = g.run(&g.policy(false), &probe);
    assert!(
        !ok && !out.contains("connected"),
        "a body without network connected: {out}"
    );
    let (ok, out) = g.run(&g.policy(true), &probe);
    assert!(
        ok && out.contains("connected"),
        "the network policy did not open it: {out}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn unix_sockets_are_closed_too() {
    let g = Ground::new("unix");
    let sock = g.path("outside/daemon.sock");
    let _listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let script = format!(
        "python3 -c 'import socket,sys; s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); print(\"connected\")' {}",
        sock.display()
    );
    let (ok, out) = g.run(&g.policy(false), &script);
    assert!(!ok && !out.contains("connected"), "{out}");
}

/// Landlock already keeps a sandboxed process out of `/proc` of processes outside its domain;
/// `harden_self` closes it again for any process, sandboxed or not.
#[cfg(target_os = "linux")]
#[test]
fn the_harness_proc_files_are_closed() {
    mnemorph_sandbox::harden_self().unwrap();
    let g = Ground::new("proc");
    let me = std::process::id();
    for file in ["environ", "mem", "maps"] {
        let (_, out) = g.run(
            &g.policy(false),
            &format!("cat /proc/{me}/{file} | head -c 64"),
        );
        assert!(
            out.is_empty() || out.contains("Permission denied"),
            "/proc/{me}/{file}: {out:?}"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn escape_syscalls_are_denied() {
    let g = Ground::new("syscalls");
    for script in ["unshare -r true", "unshare -n true"] {
        let (ok, out) = g.run(&g.policy(false), script);
        assert!(!ok, "{script}: {out}");
    }
}

/// A fork bomb is bounded, a crash leaves no core, a file cannot fill the disk.
#[cfg(target_os = "linux")]
#[test]
fn the_guard_sets_limits() {
    let g = Ground::new("limits");
    let (ok, out) = g.run(&g.policy(false), "ulimit -c; ulimit -u; ulimit -f");
    assert!(ok, "{out}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "0", "{out}");
    assert!(lines[1].parse::<u64>().is_ok_and(|n| n <= 8192), "{out}");
    assert!(lines[2].parse::<u64>().is_ok_and(|n| n <= 4 << 20), "{out}");
}

/// New namespaces widen the kernel's attack surface; neither `unshare` nor a namespace
/// `clone` gets one.
#[cfg(target_os = "linux")]
#[test]
fn no_new_namespaces() {
    let g = Ground::new("namespaces");
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    // os.fork works (a plain clone); a clone carrying CLONE_NEWUSER through libc does not.
    let script = "python3 - <<'PY'\nimport ctypes, os\nlibc = ctypes.CDLL(None, use_errno=True)\npid = os.fork()\nif pid == 0:\n    os._exit(0)\nos.waitpid(pid, 0)\nprint('fork ok')\nr = libc.unshare(0x10000000)\nprint('unshare', r, ctypes.get_errno())\nimport platform\nnr = {'x86_64': 56, 'aarch64': 220}[platform.machine()]\nr = libc.syscall(nr, 0x10000000 | 17, 0, 0, 0, 0)\nif r == 0:\n    os._exit(0)\nprint('clone', r, ctypes.get_errno())\nPY";
    let (ok, out) = g.run(&g.policy(false), script);
    assert!(ok && out.contains("fork ok"), "{out}");
    assert!(
        out.contains("unshare -1 1"),
        "unshare(CLONE_NEWUSER) must fail with EPERM: {out}"
    );
    assert!(
        out.contains("clone -1 1"),
        "clone(CLONE_NEWUSER) must fail with EPERM: {out}"
    );
}

/// Reads come from an allowlist: the home's ordinary files are as closed as its keys.
#[test]
fn the_home_is_closed_beyond_the_toolchains() {
    let g = Ground::new("home");
    let (ok, out) = g.run(&g.policy(false), "cat ~/Documents/diary.txt");
    assert!(!ok && !out.contains("dear diary"), "{out}");
}

/// Through `LaunchServices` a command could start an application outside the sandbox.
#[cfg(target_os = "macos")]
#[test]
fn no_application_is_launched_from_inside() {
    let g = Ground::new("launch");
    let (ok, out) = g.run(&g.policy(false), "open -g -a TextEdit");
    assert!(!ok, "open -a succeeded from inside the sandbox: {out}");
}
