//! What the policy names: the home's toolchains, uv's interpreters and tools among them, and
//! never a credential path.

use std::path::Path;

use mnemorph_sandbox::{HOME_CREDENTIALS, HOME_TOOLCHAINS, Policy};

#[test]
fn uv_interpreters_and_tools_are_readable_and_its_credentials_are_not() {
    let home = Path::new("/home/someone");
    let p = Policy::new(Path::new("/w"), home, Path::new("/w/.mnemorph/tmp"), false);
    for t in [".local/share/uv/python", ".local/share/uv/tools", ".pyenv"] {
        assert!(p.read.contains(&home.join(t)), "{t} is read");
    }
    // No rule names a directory above uv's credentials, nor the credentials themselves.
    for above in [
        "",
        ".local",
        ".local/share",
        ".local/share/uv",
        ".local/share/uv/credentials",
    ] {
        assert!(
            !p.read
                .iter()
                .chain(&p.write)
                .any(|r| r == &home.join(above)),
            "`{above}` is named"
        );
    }
}

/// No rule reads or writes a credential path, or a tree that holds one — on any platform, so
/// Landlock, which cannot hide a path inside a tree it reads, never has to. The toolchains are
/// never written.
#[test]
fn no_rule_holds_a_credential_and_no_toolchain_is_written() {
    let home = Path::new("/home/someone");
    let p = Policy::new(Path::new("/w"), home, Path::new("/w/.mnemorph/tmp"), false);
    for c in HOME_CREDENTIALS {
        let cred = home.join(c);
        let holders: Vec<_> = p
            .read
            .iter()
            .chain(&p.write)
            .filter(|r| cred.starts_with(r))
            .collect();
        assert!(holders.is_empty(), "{c} is held by {holders:?}");
        assert!(
            p.hidden.contains(&cred),
            "{c} is hidden where the platform can"
        );
    }
    for t in HOME_TOOLCHAINS {
        assert!(
            !p.write.iter().any(|w| home.join(t).starts_with(w)),
            "{t} is written"
        );
    }
}

/// `.cargo` and `.cache` hold credentials, so they are granted part by part: what builds read is
/// named, the trees themselves are not.
#[test]
fn cargo_and_the_cache_are_read_part_by_part() {
    let home = Path::new("/home/someone");
    let p = Policy::new(Path::new("/w"), home, Path::new("/w/.mnemorph/tmp"), false);
    for t in [
        ".cargo/bin",
        ".cargo/registry",
        ".cargo/git",
        ".cache/uv",
        ".cache/pip",
        ".cache/go-build",
        ".cache/pnpm",
        ".cache/kaginari-bench",
        ".cache/huggingface/hub",
    ] {
        assert!(p.read.contains(&home.join(t)), "{t} is read");
    }
    for tree in [".cargo", ".cache", ".cache/huggingface"] {
        assert!(!p.read.contains(&home.join(tree)), "{tree} is named whole");
    }
}
