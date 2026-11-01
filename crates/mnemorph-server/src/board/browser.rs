//! Where the human's browser can read the board's launcher page.
//!
//! A snap's confinement keeps a snap browser (Ubuntu's Chromium and Firefox) out of every hidden
//! directory under the home, so it cannot read a page in `~/.local/state/mnemorph/`, and its `/tmp`
//! is its own. What a snap may read is its own `~/snap/<snap>/common` (`$SNAP_USER_COMMON`), so a
//! launcher for a snap browser goes to `~/snap/<snap>/common/mnemorph/` (`0700`). A body's sandbox
//! reads no `~/snap`.
//!
//! The default browser is a snap when its desktop file is `<snap>_<app>.desktop` and that file's
//! `Exec` runs `/snap/bin/<snap>` (or `/snap/bin/<snap>.<app>`), or when the `Exec` of any default
//! browser's desktop file resolves under `/snap/bin/`. Anything less certain is not a snap.

use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

/// What the default browser is, for where its launcher goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Browser {
    /// A snap, by its snap name: it reads `~/snap/<name>/common` and nothing hidden in the home.
    Snap(String),
    /// Any other browser, or one not known: it reads the state directory.
    Plain,
}

/// The directory snaps run from.
const SNAP_BIN: &str = "/snap/bin/";

/// Where snapd puts the desktop files of the snaps installed, searched after the XDG data dirs.
const SNAPD_DESKTOP: &str = "/var/lib/snapd/desktop";

/// The largest desktop file read.
const ENTRY_CAP: u64 = 64 * 1024;

/// Whether `s` is a snap's name: lowercase letters, digits and single inner hyphens, at least one
/// letter, at most 40 characters.
fn snap_name(s: &str) -> bool {
    (1..=40).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && s.bytes().any(|b| b.is_ascii_lowercase())
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
}

/// Whether `s` is a snap app's name: letters, digits and single inner hyphens.
fn app_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
}

/// Whether `id` is a plain desktop file id: a file name of safe characters ending `.desktop`.
fn desktop_id(id: &str) -> bool {
    id.len() <= 255
        && id.len() > ".desktop".len()
        && id.ends_with(".desktop")
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// The snap a desktop file id names, `<snap>_<app>.desktop`, if it is shaped so.
#[must_use]
pub fn snap_of_id(id: &str) -> Option<&str> {
    if !desktop_id(id) {
        return None;
    }
    let (snap, app) = id.strip_suffix(".desktop")?.split_once('_')?;
    (snap_name(snap) && app_name(app)).then_some(snap)
}

/// The program a desktop entry's `Exec` runs: the `[Desktop Entry]` group's `Exec` key, past a
/// leading `env` and its `NAME=value` words. `None` for anything else or anything quoted.
#[must_use]
pub fn exec_program(entry: &str) -> Option<&str> {
    let mut group = false;
    let mut exec = None;
    for line in entry.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            group = line == "[Desktop Entry]";
            continue;
        }
        if !group {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim_end() == "Exec"
        {
            if exec.is_some() {
                // Two `Exec` keys: no telling which a launcher reads.
                return None;
            }
            exec = Some(value.trim());
        }
    }
    let mut words = exec?.split_ascii_whitespace();
    let mut first = words.next()?;
    if first == "env" || first == "/usr/bin/env" {
        first = words.find(|w| !w.contains('=') || w.starts_with('/'))?;
    }
    if first.contains(['"', '\'', '\\', '%', '`', '$']) {
        return None;
    }
    Some(first)
}

/// The snap that `program` runs, if it is `/snap/bin/<snap>` or `/snap/bin/<snap>.<app>`.
#[must_use]
pub fn snap_of_program(program: &Path) -> Option<String> {
    let rest = program.to_str()?.strip_prefix(SNAP_BIN)?;
    if rest.contains('/') {
        return None;
    }
    let snap = rest
        .split_once('.')
        .map_or(rest, |(s, app)| if app_name(app) { s } else { "" });
    snap_name(snap).then(|| snap.to_owned())
}

/// `program` as a launcher would run it: itself when absolute, else the first executable file of
/// that name in `path`.
fn resolve(program: &str, path: &[PathBuf]) -> Option<PathBuf> {
    if program.starts_with('/') {
        return Some(PathBuf::from(program));
    }
    if program.contains('/') || program.is_empty() {
        return None;
    }
    path.iter()
        .filter(|d| d.is_absolute())
        .map(|d| d.join(program))
        .find(|p| p.is_file())
}

/// What the browser of desktop file `id`, whose entry reads `entry` (if found), is, with `path`
/// the directories a bare program is found in.
#[must_use]
pub fn classify(id: &str, entry: Option<&str>, path: &[PathBuf]) -> Browser {
    let ran = entry
        .and_then(exec_program)
        .and_then(|p| resolve(p, path))
        .and_then(|p| snap_of_program(&p));
    match (snap_of_id(id), ran) {
        (Some(named), Some(ran)) if named == ran => Browser::Snap(ran),
        (None, Some(ran)) => Browser::Snap(ran),
        _ => Browser::Plain,
    }
}

/// The directories a desktop file is looked for in, in order: `$XDG_DATA_HOME` (or
/// `~/.local/share`), `$XDG_DATA_DIRS` (or `/usr/local/share:/usr/share`), then snapd's.
fn data_dirs(
    home: Option<&Path>,
    data_home: Option<&str>,
    data_dirs: Option<&str>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    match data_home.filter(|d| d.starts_with('/')) {
        Some(d) => dirs.push(PathBuf::from(d)),
        None => dirs.extend(home.map(|h| h.join(".local/share"))),
    }
    let system = data_dirs
        .filter(|d| !d.is_empty())
        .unwrap_or("/usr/local/share:/usr/share");
    dirs.extend(
        system
            .split(':')
            .filter(|d| d.starts_with('/'))
            .map(PathBuf::from),
    );
    dirs.push(PathBuf::from(SNAPD_DESKTOP));
    dirs
}

/// The desktop entry `id` in the first of `dirs` that holds it, read whole if small.
fn entry_of(id: &str, dirs: &[PathBuf]) -> Option<String> {
    use std::io::Read as _;
    let file = dirs
        .iter()
        .map(|d| d.join("applications").join(id))
        .find(|p| p.is_file())?;
    let mut text = String::new();
    std::fs::File::open(file)
        .ok()?
        .take(ENTRY_CAP)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// What the default browser is under this environment: `HOME`, `XDG_DATA_HOME`,
/// `XDG_DATA_DIRS` and `PATH` as given, the default's desktop id from `default`.
#[must_use]
pub fn detect_in(
    default: Option<&str>,
    home: Option<&Path>,
    data_home: Option<&str>,
    system: Option<&str>,
    path: Option<&std::ffi::OsStr>,
) -> Browser {
    let Some(id) = default.map(str::trim).filter(|id| desktop_id(id)) else {
        return Browser::Plain;
    };
    let path: Vec<PathBuf> = path
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();
    let entry = entry_of(id, &data_dirs(home, data_home, system));
    classify(id, entry.as_deref(), &path)
}

/// The first line `program args` prints, if it exits well within two seconds.
#[cfg(target_os = "linux")]
fn ask(program: &str, args: &[&str]) -> Option<String> {
    use std::io::Read as _;
    use std::process::{Command, Stdio};
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut out = String::new();
    child
        .stdout
        .take()?
        .take(4096)
        .read_to_string(&mut out)
        .ok()?;
    out.lines()
        .next()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
}

/// What the human's default browser is: its desktop id from `xdg-settings`, else `xdg-mime`'s
/// handler of `http` (Linux; anywhere else, [`Browser::Plain`]).
#[must_use]
pub fn detect() -> Browser {
    #[cfg(target_os = "linux")]
    {
        let default = ask("xdg-settings", &["get", "default-web-browser"])
            .or_else(|| ask("xdg-mime", &["query", "default", "x-scheme-handler/http"]));
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let data_home = std::env::var("XDG_DATA_HOME").ok();
        let system = std::env::var("XDG_DATA_DIRS").ok();
        let path = std::env::var_os("PATH");
        detect_in(
            default.as_deref(),
            home.as_deref().filter(|h| h.is_absolute()),
            data_home.as_deref(),
            system.as_deref(),
            path.as_deref(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        Browser::Plain
    }
}

/// This process's effective uid.
#[cfg(unix)]
fn uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// `at`, if it is a real directory (no link) this user owns.
#[cfg(unix)]
fn owned_dir(at: &Path) -> Result<std::fs::Metadata, String> {
    use std::os::unix::fs::MetadataExt as _;
    let meta = std::fs::symlink_metadata(at).map_err(|e| format!("{}: {e}", at.display()))?;
    if !meta.file_type().is_dir() || meta.uid() != uid() {
        return Err(format!(
            "{} is not a directory of this user's own (a link, or another's): refusing it",
            at.display()
        ));
    }
    Ok(meta)
}

/// `dir` made this user's private directory: created `0700` when missing (its parent must exist);
/// one of this user's own with group or other bits is tightened to `0700`, and the old mode
/// returned so the caller can say so once. A link, or another user's, is refused.
///
/// # Errors
///
/// It cannot be made or tightened, or it is a link or not this user's.
#[cfg(unix)]
pub fn private_dir(dir: &Path) -> Result<Option<u32>, String> {
    use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
    match std::fs::symlink_metadata(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(dir)
                .map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        Err(e) => return Err(format!("{}: {e}", dir.display())),
        Ok(_) => {}
    }
    let mode = owned_dir(dir)?.permissions().mode() & 0o777;
    if mode & 0o700 == mode {
        return Ok(None);
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(Some(mode))
}

/// As [`private_dir`], its missing parents made `0700` first (`$XDG_STATE_HOME/mnemorph`), and a
/// tightening said once on `say`.
///
/// # Errors
///
/// As [`private_dir`].
#[cfg(unix)]
pub fn state_dir(dir: &Path, say: impl FnOnce(&str)) -> Result<(), String> {
    use std::os::unix::fs::DirBuilderExt as _;
    if let Some(parent) = dir.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    if let Some(was) = private_dir(dir)? {
        say(&format!(
            "{} was {was:04o}: tightened to 0700 (it holds the board's nonce and sign-in pages)",
            dir.display()
        ));
    }
    Ok(())
}

/// The directory a snap browser's launcher goes in: `<home>/snap/<snap>/common/mnemorph`, made
/// `0700`. `~/snap`, `~/snap/<snap>` and its `common` are snapd's: each must already be a real
/// directory this user owns, never a link.
///
/// # Errors
///
/// The snap has no such directories, one of them is a link or another's, or `mnemorph` cannot
/// be made private.
#[cfg(unix)]
pub fn snap_dir(home: &Path, snap: &str) -> Result<PathBuf, String> {
    if !home.is_absolute() || !snap_name(snap) {
        return Err("no place a snap browser can read is known".to_owned());
    }
    let mut at = home.join("snap");
    owned_dir(&at)?;
    for part in [snap, "common"] {
        at.push(part);
        owned_dir(&at)?;
    }
    at.push("mnemorph");
    private_dir(&at)?;
    Ok(at)
}

/// Where the launcher for `browser` goes: the state directory `state` for a plain browser, the
/// snap's own `common/mnemorph` under `home` for a snap ([`snap_dir`]).
///
/// # Errors
///
/// No place the browser can read is known, or it cannot be made private.
pub fn launcher_dir(
    state: &Path,
    home: Option<&Path>,
    browser: &Browser,
) -> Result<PathBuf, String> {
    match browser {
        Browser::Plain => Ok(state.to_path_buf()),
        #[cfg(unix)]
        Browser::Snap(snap) => {
            let home =
                home.ok_or_else(|| "no home: no place a snap browser can read".to_owned())?;
            snap_dir(home, snap).map_err(|e| format!("the snap browser `{snap}`: {e}"))
        }
        #[cfg(not(unix))]
        Browser::Snap(_) => {
            let _ = home;
            Err("no place a snap browser can read is known".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAP_ENTRY: &str = "[Desktop Entry]\nVersion=1.0\nName=Chromium Web Browser\n\
        Exec=env BAMF_DESKTOP_FILE_HINT=/var/lib/snapd/desktop/applications/chromium_chromium.desktop \
        /snap/bin/chromium %U\nType=Application\n\n[Desktop Action NewWindow]\n\
        Exec=env BAMF_DESKTOP_FILE_HINT=x /snap/bin/chromium --new-window\n";
    const DEB_ENTRY: &str =
        "[Desktop Entry]\nName=Firefox\nExec=/usr/lib/firefox/firefox %u\nType=Application\n";

    #[test]
    fn a_snap_desktop_file_is_a_snap() {
        assert_eq!(snap_of_id("chromium_chromium.desktop"), Some("chromium"));
        assert_eq!(exec_program(SNAP_ENTRY), Some("/snap/bin/chromium"));
        assert_eq!(
            classify("chromium_chromium.desktop", Some(SNAP_ENTRY), &[]),
            Browser::Snap("chromium".to_owned())
        );
        // An app of its own: `/snap/bin/<snap>.<app>`.
        let app = "[Desktop Entry]\nExec=/snap/bin/firefox.geckodriver %u\n";
        assert_eq!(
            classify("firefox_geckodriver.desktop", Some(app), &[]),
            Browser::Snap("firefox".to_owned())
        );
        // A bare program found under `/snap/bin` on the path.
        let bare = "[Desktop Entry]\nExec=chromium %U\n";
        if Path::new("/snap/bin/chromium").is_file() {
            assert_eq!(
                classify(
                    "chromium.desktop",
                    Some(bare),
                    &[PathBuf::from("/snap/bin")]
                ),
                Browser::Snap("chromium".to_owned())
            );
        }
    }

    #[test]
    fn a_plain_desktop_file_is_not_a_snap() {
        assert_eq!(snap_of_id("firefox.desktop"), None);
        assert_eq!(
            classify("firefox.desktop", Some(DEB_ENTRY), &[]),
            Browser::Plain
        );
        assert_eq!(classify("google-chrome.desktop", None, &[]), Browser::Plain);
    }

    #[test]
    fn garbage_or_doubt_is_not_a_snap() {
        // Named like a snap, but no entry, or an entry that runs something else.
        assert_eq!(
            classify("chromium_chromium.desktop", None, &[]),
            Browser::Plain
        );
        assert_eq!(
            classify("chromium_chromium.desktop", Some(DEB_ENTRY), &[]),
            Browser::Plain
        );
        // Named one snap, running another.
        let other = "[Desktop Entry]\nExec=/snap/bin/firefox %u\n";
        assert_eq!(
            classify("chromium_chromium.desktop", Some(other), &[]),
            Browser::Plain
        );
        for id in [
            "",
            ".desktop",
            "_chromium.desktop",
            "Chromium_chromium.desktop",
            "chromium_.desktop",
            "chro--mium_x.desktop",
            "../evil_x.desktop",
            "chromium_chromium",
            "chromium chromium.desktop",
        ] {
            assert_eq!(snap_of_id(id), None, "{id:?}");
        }
        for entry in [
            "",
            "garbage\x00\x01",
            "Exec=/snap/bin/chromium\n",
            "[Desktop Action x]\nExec=/snap/bin/chromium\n",
            "[Desktop Entry]\nExec=\"/snap/bin/chromium\" %U\n",
            "[Desktop Entry]\nExec=/snap/bin/chromium\nExec=/usr/bin/x\n",
            "[Desktop Entry]\nExec=/snap/bin/../../usr/bin/x\n",
            "[Desktop Entry]\nExec=/snap/bin/\n",
            "[Desktop Entry]\nExec=env A=1\n",
        ] {
            assert_eq!(
                classify("chromium_chromium.desktop", Some(entry), &[]),
                Browser::Plain,
                "{entry:?}"
            );
        }
        // No default, or a strange one.
        assert_eq!(detect_in(None, None, None, None, None), Browser::Plain);
        assert_eq!(
            detect_in(Some("not a desktop id"), None, None, None, None),
            Browser::Plain
        );
    }

    #[test]
    fn the_default_is_found_in_the_data_dirs() {
        let home = tempfile::tempdir().unwrap();
        let apps = home.path().join(".local/share/applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(apps.join("chromium_chromium.desktop"), SNAP_ENTRY).unwrap();
        std::fs::write(apps.join("firefox.desktop"), DEB_ENTRY).unwrap();
        let empty = home.path().join("none");
        let system = empty.to_str().unwrap();
        assert_eq!(
            detect_in(
                Some("chromium_chromium.desktop\n"),
                Some(home.path()),
                None,
                Some(system),
                None
            ),
            Browser::Snap("chromium".to_owned())
        );
        assert_eq!(
            detect_in(
                Some("firefox.desktop"),
                Some(home.path()),
                None,
                Some(system),
                None
            ),
            Browser::Plain
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_snap_launcher_goes_to_the_snaps_own_common_directory() {
        use std::os::unix::fs::PermissionsExt as _;
        let home = tempfile::tempdir().unwrap();
        let state = home.path().join(".local/state/mnemorph");
        let snap = Browser::Snap("chromium".to_owned());
        // No `~/snap/chromium/common`: no place is known.
        assert!(launcher_dir(&state, Some(home.path()), &snap).is_err());
        assert!(launcher_dir(&state, None, &snap).is_err());
        let common = home.path().join("snap/chromium/common");
        std::fs::create_dir_all(&common).unwrap();
        let dir = launcher_dir(&state, Some(home.path()), &snap).unwrap();
        assert_eq!(dir, common.join("mnemorph"));
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        // A loose one is tightened.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        launcher_dir(&state, Some(home.path()), &snap).unwrap();
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        // A plain browser: the state directory.
        assert_eq!(
            launcher_dir(&state, Some(home.path()), &Browser::Plain).unwrap(),
            state
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_the_snap_chain_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = home.path().join("elsewhere");
        std::fs::create_dir_all(elsewhere.join("common/mnemorph")).unwrap();
        std::fs::create_dir_all(home.path().join("snap")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, home.path().join("snap/chromium")).unwrap();
        let e = snap_dir(home.path(), "chromium").unwrap_err();
        assert!(e.contains("refusing"), "{e}");
        // The last link too.
        std::fs::remove_file(home.path().join("snap/chromium")).unwrap();
        std::fs::create_dir_all(home.path().join("snap/chromium/common")).unwrap();
        std::os::unix::fs::symlink(
            elsewhere.join("common/mnemorph"),
            home.path().join("snap/chromium/common/mnemorph"),
        )
        .unwrap();
        let e = snap_dir(home.path(), "chromium").unwrap_err();
        assert!(e.contains("refusing"), "{e}");
    }

    #[cfg(unix)]
    #[test]
    fn a_loose_state_directory_is_tightened_once() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("state/mnemorph");
        let mut said = Vec::new();
        state_dir(&state, |s| said.push(s.to_owned())).unwrap();
        assert!(said.is_empty(), "made private: nothing to say");
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&state), 0o700);
        std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o775)).unwrap();
        state_dir(&state, |s| said.push(s.to_owned())).unwrap();
        assert_eq!(mode(&state), 0o700);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(
            said[0].contains("0775") && said[0].contains("0700"),
            "{said:?}"
        );
        state_dir(&state, |s| said.push(s.to_owned())).unwrap();
        assert_eq!(said.len(), 1, "said once");
        // A link in its place is refused, never followed.
        let link = root.path().join("state/linked");
        std::os::unix::fs::symlink(&state, &link).unwrap();
        assert!(state_dir(&link, |_| {}).is_err());
    }
}
