//! `mnemorph-guard [--read P]… [--write P]… [--rules FILE] [--network] [--die-with-parent] -- PROGRAM ARGS…`
//!
//! `--rules` reads `r`/`w` and a path, each NUL-terminated, from FILE and removes it.
//!
//! Restricts its own process with landlock and seccomp, then execs PROGRAM in its place, so the
//! command and everything it starts inherit the restriction. `--probe` restricts and prints the
//! status line instead of running anything. `--die-with-parent` has the kernel kill PROGRAM when
//! the thread that started the guard ends, so a long-lived server never outlives the harness. Exit 125: the restriction could not be enforced,
//! nothing ran; exit 127: PROGRAM could not be executed.

#[cfg(target_os = "linux")]
fn main() {
    use std::os::unix::process::CommandExt as _;
    use std::path::PathBuf;
    use std::process::{Command, exit};

    use mnemorph_sandbox::linux::{Rules, restrict};

    let mut rules = Rules::default();
    let mut probe = false;
    let mut die_with_parent = false;
    let parent = rustix::process::getppid();
    let mut args = std::env::args_os().skip(1);
    let mut program: Vec<std::ffi::OsString> = Vec::new();
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--read") => rules.read.extend(args.next().map(PathBuf::from)),
            Some("--write") => rules.write.extend(args.next().map(PathBuf::from)),
            Some("--network") => rules.network = true,
            Some("--rules") => {
                let Some(file) = args.next().map(PathBuf::from) else {
                    eprintln!("mnemorph-guard: --rules needs a file");
                    exit(125);
                };
                if let Err(e) = read_rules(&file, &mut rules) {
                    eprintln!("mnemorph-guard: the rules file: {e}; nothing ran");
                    exit(125);
                }
            }
            Some("--probe") => probe = true,
            Some("--die-with-parent") => die_with_parent = true,
            Some("--") => {
                program.extend(args.by_ref());
                break;
            }
            _ => {
                eprintln!("mnemorph-guard: unknown argument {}", a.to_string_lossy());
                exit(125);
            }
        }
    }
    if die_with_parent {
        // Set before the parent can be checked, then checked: a parent gone in between is caught.
        if let Err(e) =
            rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::KILL))
        {
            eprintln!("mnemorph-guard: the parent-death signal: {e}; nothing ran");
            exit(125);
        }
        if rustix::process::getppid() != parent {
            exit(125);
        }
    }
    let status = match restrict(&rules) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mnemorph-guard: {e}; nothing ran");
            exit(125);
        }
    };
    if probe {
        println!("{status}");
        return;
    }
    let Some((prog, rest)) = program.split_first() else {
        eprintln!("mnemorph-guard: no command after --");
        exit(125);
    };
    let err = Command::new(prog).args(rest).exec();
    eprintln!(
        "mnemorph-guard: cannot run {}: {err}",
        prog.to_string_lossy()
    );
    exit(127);
}

/// Read the rules file into `rules`, then remove it: it is read once, before any restriction.
#[cfg(target_os = "linux")]
fn read_rules(
    file: &std::path::Path,
    rules: &mut mnemorph_sandbox::linux::Rules,
) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt as _;
    let bytes = std::fs::read(file)?;
    std::fs::remove_file(file)?;
    let mut fields = bytes.split(|b| *b == 0);
    while let (Some(kind), Some(path)) = (fields.next(), fields.next()) {
        let path = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(path));
        match kind {
            b"r" => rules.read.push(path),
            b"w" => rules.write.push(path),
            b"" => break,
            _ => return Err(std::io::Error::other("a rule is neither r nor w")),
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("mnemorph-guard runs on Linux only");
    std::process::exit(125);
}
