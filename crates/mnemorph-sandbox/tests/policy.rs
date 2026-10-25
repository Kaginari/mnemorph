//! What the policy names: the home's toolchains, uv's interpreters and tools among them, and
//! never a credential path.

use std::path::Path;

use mnemorph_sandbox::Policy;

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
