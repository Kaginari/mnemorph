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
        ".config/git/config",
        ".gradle/caches",
        ".m2/repository",
    ] {
        assert!(p.read.contains(&home.join(t)), "{t} is read");
    }
    for tree in [
        ".cargo",
        ".cache",
        ".cache/huggingface",
        ".config/git",
        ".gradle",
        ".m2",
        ".nvm",
        "go",
        "go/src",
    ] {
        assert!(!p.read.contains(&home.join(tree)), "{tree} is named whole");
    }
}

/// A home made on disk for the tests that need real paths.
fn temp_home(name: &str) -> std::path::PathBuf {
    let home = std::env::temp_dir().join(format!("mnemorph-policy-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    home.canonicalize().unwrap()
}

/// nvm's Node versions are read part by part: `bin`, `lib`, `include` — never `etc/npmrc`.
#[test]
fn each_nvm_node_is_read_without_its_npmrc() {
    let home = temp_home("nvm");
    for d in ["bin", "lib", "include", "etc"] {
        std::fs::create_dir_all(home.join(".nvm/versions/node/v22.0.0").join(d)).unwrap();
    }
    let p = Policy::new(Path::new("/w"), &home, Path::new("/w/.mnemorph/tmp"), false);
    let node = home.join(".nvm/versions/node/v22.0.0");
    for d in ["bin", "lib", "include"] {
        assert!(p.read.contains(&node.join(d)), "{d} is read");
    }
    let npmrc = node.join("etc/npmrc");
    assert!(
        !p.read.iter().any(|r| npmrc.starts_with(r)),
        "etc/npmrc is held"
    );
    std::fs::remove_dir_all(&home).unwrap();
}

/// A toolchain path linked onto, inside or above a credential grants nothing: `.cache/uv` made a
/// link to `.ssh`, `.cache/pip` to the home itself.
#[test]
fn a_toolchain_linked_to_a_credential_is_refused() {
    let home = temp_home("links");
    std::fs::create_dir_all(home.join(".ssh")).unwrap();
    std::fs::create_dir_all(home.join(".cache")).unwrap();
    std::fs::create_dir_all(home.join(".cargo/registry")).unwrap();
    std::os::unix::fs::symlink(home.join(".ssh"), home.join(".cache/uv")).unwrap();
    std::os::unix::fs::symlink(&home, home.join(".cache/pip")).unwrap();
    let p = Policy::new(Path::new("/w"), &home, Path::new("/w/.mnemorph/tmp"), false);
    for t in [".cache/uv", ".cache/pip"] {
        assert!(!p.read.contains(&home.join(t)), "{t} is read");
        assert!(p.refused.contains(&home.join(t)), "{t} is not noted");
    }
    assert!(p.read.contains(&home.join(".cargo/registry")));
    let granted = Policy::existing(&p.read);
    assert!(
        !granted.iter().any(|g| home.join(".ssh").starts_with(g)),
        "a rule holds .ssh: {granted:?}"
    );
    std::fs::remove_dir_all(&home).unwrap();
}
