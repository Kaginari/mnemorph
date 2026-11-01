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

/// Credentials inside the toolchain trees stay closed while the caches beside them read: the
/// crates.io token beside cargo's registry, Hugging Face's token beside uv's cache, git's
/// credential store beside its config, Maven's and Gradle's passwords beside their caches, an
/// npmrc beside nvm's Node.
#[test]
fn toolchain_credentials_cannot_be_read_but_their_caches_can() {
    let s = sandbox();
    let status = s.status(false);
    if status.starts_with("none") {
        eprintln!("skipped: {status}");
        return;
    }
    let g = Ground::new("toolchains");
    for (file, text) in [
        (".cargo/credentials.toml", "token = \"CARGO TOKEN\"\n"),
        (".cache/huggingface/token", "hf_TOKEN\n"),
        (".config/git/credentials", "https://u:TOKEN@host\n"),
        (".m2/settings.xml", "<password>TOKEN</password>\n"),
        (".gradle/gradle.properties", "repoPassword=TOKEN\n"),
        (".nvm/versions/node/v1/etc/npmrc", "//r/:_authToken=TOKEN\n"),
        (".cargo/registry/index/entry", "registry\n"),
        (".cache/uv/CACHEDIR.TAG", "uv cache\n"),
        (".config/git/config", "git config\n"),
        (".m2/repository/entry", "maven\n"),
        (".nvm/versions/node/v1/bin/node", "node\n"),
    ] {
        let path = g.path("home").join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    // A link inside a granted cache leads to the keys: Landlock resolves it when the file opens.
    std::os::unix::fs::symlink(g.path("home/.ssh"), g.path("home/.cache/uv/x")).unwrap();
    for script in [
        "cat ~/.cache/uv/x/id_ed25519",
        "cat ~/.cargo/credentials.toml",
        "cat ~/.cache/huggingface/token",
        "cat ~/.config/git/credentials",
        "cat ~/.m2/settings.xml",
        "cat ~/.gradle/gradle.properties",
        "cat ~/.nvm/versions/node/v1/etc/npmrc",
        "ls ~/.cache/huggingface",
        "ls ~/.cargo",
    ] {
        let (ok, out) = g.run(&g.policy(false), script);
        assert!(
            !ok && !out.contains("TOKEN")
                && !out.contains("PRIVATE KEY")
                && !out.lines().any(|l| l == "credentials.toml" || l == "token"),
            "{script}: {out}"
        );
    }
    let (ok, out) = g.run(
        &g.policy(false),
        "cat ~/.cargo/registry/index/entry ~/.cache/uv/CACHEDIR.TAG ~/.config/git/config \
         ~/.m2/repository/entry ~/.nvm/versions/node/v1/bin/node",
    );
    assert!(ok, "{out}");
    assert_eq!(out, "registry\nuv cache\ngit config\nmaven\nnode\n");
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

/// No process leaves the command's group: `setsid` and `setpgid` are refused, so killing the
/// group ends everything the command started; a plain background job still runs.
#[cfg(target_os = "linux")]
#[test]
fn nothing_leaves_the_process_group() {
    let g = Ground::new("pgroup");
    for script in ["setsid true", "setsid -f sh -c 'echo escaped'; sleep 0.2"] {
        let (_, out) = g.run(&g.policy(false), script);
        assert!(
            out.contains("not permitted") && !out.contains("escaped"),
            "{script}: {out}"
        );
    }
    let (ok, out) = g.run(&g.policy(false), "(echo bg) & wait");
    assert!(ok && out.contains("bg"), "{out}");
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

/// The variable a stand-in harness carries its fake secret in.
const FAKE_SECRET: &str = "MNEMORPH_TEST_FAKE_SECRET";

/// What a command finds of `secret` in `/proc`: it opens `/proc/$PPID/environ`,
/// `/proc/<harness>/environ` and `/proc/<harness>/cmdline`, then every `/proc/*/environ`, and
/// prints `open F` for each environment that opens and `LEAK F` for each file holding the secret
/// — never a file's contents. Its own environment is scrubbed, as the shell tool scrubs it.
#[cfg(target_os = "linux")]
fn probe_proc(g: &Ground, sandbox: &Sandbox, secret: &str) -> String {
    let me = std::process::id();
    let script = format!(
        "for f in /proc/$PPID/environ /proc/{me}/environ /proc/{me}/cmdline; do \
           if grep -qaF \"$1\" \"$f\" 2>/dev/null; then echo \"LEAK $f\"; fi; \
         done; \
         for f in /proc/[0-9]*/environ; do \
           [ \"$f\" = /proc/$$/environ ] && continue; \
           (: < \"$f\") 2>/dev/null && echo \"open $f\"; \
           if grep -qaF \"$1\" \"$f\" 2>/dev/null; then echo \"LEAK $f\"; fi; \
         done; \
         echo done"
    );
    let out = sandbox
        .command(
            &g.policy(false),
            "/bin/bash",
            &["--noprofile", "--norc", "-c", &script, "probe", secret],
        )
        .current_dir(g.path("world"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", g.path("home"))
        .env("TMPDIR", g.path("tmp"))
        .output()
        .expect("the command starts");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A body cannot read the harness's environment, where a provider key lives. The test re-runs
/// itself as a stand-in harness whose environment carries a fake secret. Under the guard,
/// landlock alone keeps every other process's environment shut; once the harness is hardened
/// as `mnemorph` and `mnemorphd` harden themselves, even an unguarded command cannot read it.
#[cfg(target_os = "linux")]
#[test]
fn a_body_cannot_read_the_harness_environment() {
    let Some(secret) = std::env::var_os(FAKE_SECRET) else {
        let secret = format!("fake-{}-not-a-key", std::process::id());
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "a_body_cannot_read_the_harness_environment",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(FAKE_SECRET, &secret)
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.status.success(), "{text}");
        assert!(
            text.contains("1 passed"),
            "the stand-in did not run: {text}"
        );
        return;
    };
    let secret = secret.into_string().unwrap();
    let g = Ground::new("environ");
    let guarded = probe_proc(&g, &sandbox(), &secret);
    assert!(guarded.ends_with("done\n"), "{guarded}");
    assert!(
        !guarded.contains("open ") && !guarded.contains("LEAK"),
        "a guarded command read a process's environment: {guarded}"
    );
    mnemorph_sandbox::harden_self().unwrap();
    let unguarded = probe_proc(&g, &Sandbox::None("the test".into()), &secret);
    assert!(unguarded.ends_with("done\n"), "{unguarded}");
    assert!(
        !unguarded.contains("LEAK"),
        "an unguarded command read the hardened harness's environment: {unguarded}"
    );
}

/// The harness's own git calls start without its secrets: a stand-in harness carrying a fake
/// secret runs `env` through a git alias started by [`mnemorph_sandbox::git`].
#[test]
fn the_harness_git_carries_no_key() {
    let Some(secret) = std::env::var_os(FAKE_SECRET) else {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "the_harness_git_carries_no_key",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(
                FAKE_SECRET,
                format!("fake-{}-not-a-key", std::process::id()),
            )
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{text}");
        assert!(
            text.contains("1 passed"),
            "the stand-in did not run: {text}"
        );
        return;
    };
    let secret = secret.into_string().unwrap();
    let out = mnemorph_sandbox::git()
        .args(["-c", "alias.showenv=!env", "showenv"])
        .output()
        .expect("git runs");
    let env = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success() && env.contains("PATH="), "{env}");
    assert!(!env.contains(&secret), "git's child got the secret");
    assert!(
        !env.contains(FAKE_SECRET),
        "git's child got the secret's variable"
    );
}

/// On macOS a body cannot read the harness's environment through sysctl: `ps -E`, which reads
/// `kern.procargs2`, finds no fake secret in a stand-in harness.
#[cfg(target_os = "macos")]
#[test]
fn a_body_cannot_read_the_harness_environment_through_sysctl() {
    let Some(secret) = std::env::var_os(FAKE_SECRET) else {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "a_body_cannot_read_the_harness_environment_through_sysctl",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(
                FAKE_SECRET,
                format!("fake-{}-not-a-key", std::process::id()),
            )
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{text}");
        assert!(
            text.contains("1 passed"),
            "the stand-in did not run: {text}"
        );
        return;
    };
    let secret = secret.into_string().unwrap();
    let g = Ground::new("sysctl");
    let me = std::process::id();
    let (_, out) = g.run(
        &g.policy(false),
        &format!("ps -wwwE -o command= -p {me} | grep -cF '{secret}'; echo done"),
    );
    assert!(out.contains("done"), "{out}");
    assert!(
        out.starts_with("0\n"),
        "ps read the harness's environment: {out}"
    );
}

/// The board's control nonce and its launcher pages live in the human's state directory: a body —
/// even one with the network, which may connect to the board's socket — reads neither, so it can
/// neither drive the socket nor take a key.
#[test]
fn the_boards_nonce_and_launcher_cannot_be_read() {
    let g = Ground::new("board");
    let state = g.path("home/.local/state/mnemorph");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("board.ctl"), "NONCE-SECRET\n").unwrap();
    fs::write(state.join("open-0123456789abcdef.html"), "KEY-SECRET\n").unwrap();
    for network in [false, true] {
        for file in ["board.ctl", "open-0123456789abcdef.html"] {
            let (ok, out) = g.run(
                &g.policy(network),
                &format!("cat {}", state.join(file).display()),
            );
            assert!(
                !ok && !out.contains("SECRET"),
                "network {network}, {file}: {out}"
            );
        }
        let (_, out) = g.run(&g.policy(network), &format!("ls {}", state.display()));
        assert!(!out.contains("open-"), "network {network}: {out}");
    }
}
