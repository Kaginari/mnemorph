//! `mnemorph-guard [--read P]… [--write P]… [--network] -- PROGRAM ARGS…`
//!
//! Restricts its own process with landlock and seccomp, then execs PROGRAM in its place, so the
//! command and everything it starts inherit the restriction. `--probe` restricts and prints the
//! status line instead of running anything. Exit 125: the restriction could not be enforced,
//! nothing ran; exit 127: PROGRAM could not be executed.

#[cfg(target_os = "linux")]
fn main() {
    use std::os::unix::process::CommandExt as _;
    use std::path::PathBuf;
    use std::process::{Command, exit};

    use mnemorph_sandbox::linux::{Rules, restrict};

    let mut rules = Rules::default();
    let mut probe = false;
    let mut args = std::env::args_os().skip(1);
    let mut program: Vec<std::ffi::OsString> = Vec::new();
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--read") => rules.read.extend(args.next().map(PathBuf::from)),
            Some("--write") => rules.write.extend(args.next().map(PathBuf::from)),
            Some("--network") => rules.network = true,
            Some("--probe") => probe = true,
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

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("mnemorph-guard runs on Linux only");
    std::process::exit(125);
}
