//! What the policy names: the home's toolchains, uv's interpreters and tools among them, and
//! never a credential path.

use std::path::Path;

use mnemorph_sandbox::{HOME_CREDENTIALS, Policy};

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

/// No rule reads or writes a credential path, or a tree that holds one — those inside a readable
/// toolchain aside, named here: landlock cannot hide them, and every platform that can hides
/// them. The toolchains are never written.
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
        let inside = [
            ".cargo/credentials",
            ".cargo/credentials.toml",
            ".cache/huggingface",
            ".cache/git",
        ];
        if inside.contains(c) {
            assert_eq!(holders.len(), 1, "{c}: {holders:?}");
            assert!(
                p.write.iter().all(|w| !cred.starts_with(w)),
                "{c} is written"
            );
            assert!(
                p.hidden.contains(&cred),
                "{c} is hidden where the platform can"
            );
        } else {
            assert!(holders.is_empty(), "{c} is held by {holders:?}");
        }
    }
    for t in [".local/share/uv/python", ".local/share/uv/tools"] {
        assert!(
            !p.write.iter().any(|w| home.join(t).starts_with(w)),
            "{t} is written"
        );
    }
}
