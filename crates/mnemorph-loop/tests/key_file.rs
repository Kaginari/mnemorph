//! A provider's key from its key file: the variable first, the file when it is unset, a hole
//! naming both when neither gives one — and every unsafe file refused, its contents never shown.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use mnemorph_loop::config::Provider;
use mnemorph_loop::key::{inspect, resolve};

/// A fake key: never a real one, searched for in every message.
const FAKE: &str = "sk-or-v1-fakefakefake0123456789abcdef";

struct Home {
    dir: tempfile::TempDir,
}

impl Home {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    /// A key file under this home, holding `text`, with `mode`.
    fn key(&self, name: &str, text: &str, mode: u32) -> PathBuf {
        let p = self.path().join(name);
        fs::write(&p, text).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
        p
    }

    fn env(&self, var: Option<&str>) -> impl Fn(&str) -> Option<String> + use<> {
        let home = self.path().display().to_string();
        let var = var.map(str::to_owned);
        move |name: &str| match name {
            "HOME" => Some(home.clone()),
            "OPENROUTER_API_KEY" => var.clone(),
            _ => None,
        }
    }
}

fn provider(key_env: Option<&str>, key_file: Option<&str>) -> Provider {
    Provider {
        kind: "openai".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        key_env: key_env.map(str::to_owned),
        key_file: key_file.map(str::to_owned),
    }
}

fn no_leak(s: &str) {
    assert!(!s.contains(FAKE), "the key leaked: {s}");
    assert!(!s.contains("fakefakefake"), "part of the key leaked: {s}");
}

/// The error `resolve` gives for `p`, with no key bytes in it.
fn hole(h: &Home, p: &Provider, worlds: &[PathBuf]) -> String {
    let e = resolve("openrouter", p, &h.env(None), worlds).unwrap_err();
    no_leak(&e);
    e
}

#[test]
fn the_variable_wins_over_the_file() {
    let h = Home::new();
    h.key("k.key", &format!("{FAKE}\n"), 0o600);
    let r = resolve(
        "openrouter",
        &provider(Some("OPENROUTER_API_KEY"), Some("~/k.key")),
        &h.env(Some("from-the-env")),
        &[],
    )
    .unwrap()
    .unwrap();
    assert_eq!(r.source, "env OPENROUTER_API_KEY");
    assert_eq!(r.key.expose(), "from-the-env");
}

#[test]
fn the_file_is_read_when_the_variable_is_unset() {
    let h = Home::new();
    h.key("k.key", &format!("  {FAKE}\n\n"), 0o600);
    let r = resolve(
        "openrouter",
        &provider(Some("OPENROUTER_API_KEY"), Some("~/k.key")),
        &h.env(None),
        &[],
    )
    .unwrap()
    .unwrap();
    assert_eq!(r.source, "file ~/k.key");
    no_leak(&format!("{r:?}"));
    assert_eq!(r.key.expose(), FAKE, "trimmed");
    // An empty variable is not a key: the file answers.
    let r = resolve(
        "openrouter",
        &provider(Some("OPENROUTER_API_KEY"), Some("~/k.key")),
        &h.env(Some("")),
        &[],
    )
    .unwrap()
    .unwrap();
    assert_eq!(r.source, "file ~/k.key");
    // Read-only by its owner is as good.
    h.key("ro.key", FAKE, 0o400);
    assert!(
        resolve(
            "openrouter",
            &provider(None, Some("~/ro.key")),
            &h.env(None),
            &[]
        )
        .is_ok()
    );
}

#[test]
fn neither_gives_a_hole_naming_both() {
    let h = Home::new();
    let e = hole(
        &h,
        &provider(Some("OPENROUTER_API_KEY"), Some("~/missing.key")),
        &[],
    );
    assert!(
        e.starts_with("OPENROUTER_API_KEY is not set and ~/missing.key is unreadable: "),
        "{e}"
    );
    let e = hole(&h, &provider(Some("OPENROUTER_API_KEY"), None), &[]);
    assert_eq!(
        e,
        "OPENROUTER_API_KEY is not set: provider `openrouter` reads its key from it"
    );
    assert!(
        resolve("x", &provider(None, None), &h.env(None), &[])
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_file_others_may_read_is_refused_naming_the_fix() {
    let h = Home::new();
    h.key("k.key", FAKE, 0o644);
    let e = hole(&h, &provider(None, Some("~/k.key")), &[]);
    assert!(
        e.contains("mode is 0644") && e.contains("chmod 600 ~/k.key"),
        "{e}"
    );
    h.key("g.key", FAKE, 0o640);
    assert!(hole(&h, &provider(None, Some("~/g.key")), &[]).contains("chmod 600"));
}

#[test]
fn a_symbolic_link_is_refused() {
    let h = Home::new();
    let real = h.key("real.key", FAKE, 0o600);
    std::os::unix::fs::symlink(&real, h.path().join("link.key")).unwrap();
    let e = hole(&h, &provider(None, Some("~/link.key")), &[]);
    assert!(e.contains("symbolic link"), "{e}");
}

#[test]
fn a_directory_or_a_fifo_is_refused() {
    let h = Home::new();
    fs::create_dir(h.path().join("dir.key")).unwrap();
    fs::set_permissions(h.path().join("dir.key"), fs::Permissions::from_mode(0o600)).unwrap();
    let e = hole(&h, &provider(None, Some("~/dir.key")), &[]);
    assert!(e.contains("not a regular file"), "{e}");
    rustix::fs::mknodat(
        rustix::fs::CWD,
        h.path().join("fifo.key"),
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::from_raw_mode(0o600),
        0,
    )
    .unwrap();
    let e = hole(&h, &provider(None, Some("~/fifo.key")), &[]);
    assert!(e.contains("not a regular file"), "{e}");
}

#[test]
fn a_file_inside_a_world_is_refused() {
    let h = Home::new();
    fs::create_dir(h.path().join("world")).unwrap();
    h.key("world/k.key", FAKE, 0o600);
    let world = h.path().join("world").canonicalize().unwrap();
    let e = hole(&h, &provider(None, Some("~/world/k.key")), &[world]);
    assert!(e.contains("inside the world"), "{e}");
}

#[test]
fn a_large_or_empty_file_is_refused() {
    let h = Home::new();
    h.key("big.key", &"a".repeat(4097), 0o600);
    let e = hole(&h, &provider(None, Some("~/big.key")), &[]);
    assert!(e.contains("4097 bytes"), "{e}");
    h.key("empty.key", " \n\t\n", 0o600);
    let e = hole(&h, &provider(None, Some("~/empty.key")), &[]);
    assert!(e.contains("empty"), "{e}");
    h.key("two.key", &format!("{FAKE}\nsecond"), 0o600);
    let e = hole(&h, &provider(None, Some("~/two.key")), &[]);
    assert!(e.contains("more than one line"), "{e}");
}

#[test]
fn a_relative_path_is_refused() {
    let h = Home::new();
    let e = hole(&h, &provider(None, Some("k.key")), &[]);
    assert!(e.contains("not an absolute path"), "{e}");
}

#[test]
fn inspect_tells_the_mode_never_the_key() {
    let h = Home::new();
    h.key("k.key", FAKE, 0o600);
    let home = h.path().display().to_string();
    let c = inspect("~/k.key", Some(&home), &[]).unwrap();
    assert_eq!(c.mode, 0o600);
    no_leak(&format!("{c:?}"));
    h.key("open.key", FAKE, 0o644);
    let e = inspect("~/open.key", Some(&home), &[]).unwrap_err();
    no_leak(&e);
    assert!(e.contains("chmod 600"));
}
