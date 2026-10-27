//! Where a path lands, judged by whole segments after `..` and symlinks are resolved.

use std::path::{Component, Path, PathBuf};

use mnemorph_races::{OutwardKind, ToolClass};

use super::{Place, Rule, Tally};

/// What a command does to a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Act {
    Read,
    Modify,
    Delete,
}

/// Where a path lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Spot {
    /// Inside the world; `root` when it is the world's root itself (or all of its contents).
    Inside {
        /// The world's root itself, or all of its contents.
        root: bool,
        /// Inside `.git` or the world's records in `.mnemorph` (but `tmp/`, `intel/` and `report/`).
        records: bool,
    },
    /// A device file.
    Device(Device),
    /// A credential: a key, a token store, a password vault.
    Credential(PathBuf),
    /// Outside the world; `danger` when it is `/`, a top-level directory, or an ancestor of the
    /// world or of the home.
    Outside { danger: bool, path: PathBuf },
    /// Known only at run time.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Device {
    /// `/dev/null`, the standard streams, `/dev/fd/*`, the random sources, the terminal.
    Harmless,
    /// `/dev/tcp/*`, `/dev/udp/*`: bash opens a socket.
    Network,
    /// Any other device: disks above all.
    Other,
}

use mnemorph_sandbox::{HOME_CREDENTIALS, SYSTEM_CREDENTIALS};

/// Join `text` onto `cwd` the way the shell would, `..` and `.` folded lexically.
pub(crate) fn join(cwd: Option<&Path>, text: &str) -> Option<PathBuf> {
    let raw = Path::new(text);
    let full = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        cwd?.join(raw)
    };
    let mut out = PathBuf::from("/");
    for c in full.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(n) => out.push(n),
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    Some(out)
}

/// Resolve symlinks along the longest prefix of `path` that exists, keep the rest as written.
pub(crate) fn real(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut rest = Vec::new();
    loop {
        if let Ok(canon) = existing.canonicalize() {
            let mut out = canon;
            for r in rest.iter().rev() {
                out.push(r);
            }
            return out;
        }
        match (
            existing.file_name().map(ToOwned::to_owned),
            existing.parent(),
        ) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Where `text` lands when the command runs in `cwd`.
pub(crate) fn locate(text: Option<&str>, cwd: Option<&Path>, place: &Place) -> Spot {
    let Some(text) = text else {
        return Spot::Unknown;
    };
    // A glob covers the contents of the directory before its first wild segment.
    let Some(lexical) = join(cwd, &globless(text)) else {
        return Spot::Unknown;
    };
    if let Some(device) = device(&lexical) {
        return Spot::Device(device);
    }
    let path = real(&lexical);
    let home = real(&place.home);
    let world = real(&place.world);
    // Danger and credentials are judged on the path as written and as resolved, each with the
    // system's aliases removed: on macOS `/etc` is `/private/etc` and `/home` is
    // `/System/Volumes/Data/home`, and either spelling must be refused alike.
    let forms = [unaliased(&lexical), unaliased(&path)];
    let bare_home = unaliased(&home);
    if is_credential(&path, &home) || forms.iter().any(|f| is_credential(f, &bare_home)) {
        return Spot::Credential(path);
    }
    if path.starts_with(&world) {
        let rel = path.strip_prefix(&world).unwrap_or(&path);
        let records = rel.starts_with(".git")
            || (rel.starts_with(".mnemorph")
                && !rel.starts_with(".mnemorph/tmp")
                && !rel.starts_with(".mnemorph/report")
                && !rel.starts_with(".mnemorph/intel"));
        return Spot::Inside {
            root: path == world,
            records,
        };
    }
    let top_level = forms.iter().any(|f| f.components().count() <= 2);
    let ancestor = [&world, &home].iter().any(|anchor| {
        anchor.starts_with(&path) || forms.iter().any(|f| unaliased(anchor).starts_with(f))
    });
    Spot::Outside {
        danger: top_level || ancestor,
        path,
    }
}

/// The system's own aliases for its directories, longest first.
const ALIASES: &[&str] = &["/System/Volumes/Data", "/private"];

/// `path` without a leading system alias: `/private/etc/hosts` is `/etc/hosts`.
fn unaliased(path: &Path) -> PathBuf {
    for alias in ALIASES {
        if let Ok(rest) = path.strip_prefix(alias)
            && !rest.as_os_str().is_empty()
        {
            return Path::new("/").join(rest);
        }
    }
    path.to_path_buf()
}

/// A glob path as the one path it stands for. A bare `*` stands for all of its directory, so it
/// is judged as that directory; a narrower pattern (`*.log`) stands for a child of it.
fn globless(text: &str) -> String {
    let Some(i) = text.find(['*', '?', '[']) else {
        return text.to_owned();
    };
    let start = text[..i].rfind('/').map_or(0, |s| s + 1);
    let end = text[i..].find('/').map_or(text.len(), |e| i + e);
    let dir = match &text[..start] {
        "" => ".",
        "/" => "/",
        d => d.trim_end_matches('/'),
    };
    match &text[start..end] {
        "*" | "**" | ".*" => dir.to_owned(),
        _ => format!("{dir}/glob"),
    }
}

fn device(path: &Path) -> Option<Device> {
    let rest = path.strip_prefix("/dev").ok()?;
    let s = rest.to_string_lossy();
    Some(match s.as_ref() {
        "null" | "zero" | "stdin" | "stdout" | "stderr" | "tty" | "random" | "urandom" => {
            Device::Harmless
        }
        _ if s.starts_with("fd/") => Device::Harmless,
        _ if s.starts_with("tcp/") || s.starts_with("udp/") => Device::Network,
        _ => Device::Other,
    })
}

fn is_credential(path: &Path, home: &Path) -> bool {
    if SYSTEM_CREDENTIALS.iter().any(|c| path.starts_with(c)) {
        return true;
    }
    path.strip_prefix(home)
        .is_ok_and(|rel| HOME_CREDENTIALS.iter().any(|c| rel.starts_with(c)))
}

/// Raise the tally for what `act` does at `spot`.
pub(crate) fn judge(spot: &Spot, act: Act, what: &str, place: &Place, t: &mut Tally) {
    match (spot, act) {
        (Spot::Credential(p), _) => {
            t.refuse(
                Rule::Credential,
                format!("{what} touches a credential: {}", p.display()),
            );
        }
        (Spot::Inside { .. } | Spot::Outside { .. } | Spot::Unknown, Act::Read)
        | (Spot::Device(Device::Harmless), _) => {}
        (Spot::Device(Device::Network), _) => {
            t.raise(ToolClass::Outward, format!("{what} opens a network socket"));
        }
        (Spot::Device(Device::Other), Act::Read) => {
            t.raise(ToolClass::Write, format!("{what} reads a device"));
        }
        (Spot::Device(Device::Other), _) => {
            t.refuse(Rule::Device, format!("{what} writes a device"));
        }
        // Written in place, an object the attic keeps by hard link would change in the attic too:
        // changing the repository or the records asks, as deleting them does.
        (Spot::Inside { records: true, .. }, Act::Modify) => {
            t.raise(
                ToolClass::Destructive,
                format!("{what} changes the repository's history or the world's records in place"),
            );
        }
        (Spot::Inside { records: true, .. }, Act::Delete) => {
            t.raise(
                ToolClass::Destructive,
                format!("{what} deletes the repository's history or the world's records"),
            );
        }
        (Spot::Inside { root: true, .. }, Act::Delete) => {
            t.raise(
                ToolClass::Destructive,
                format!("{what} deletes the whole world"),
            );
        }
        (Spot::Inside { .. }, _) => t.raise(ToolClass::Write, format!("{what} changes the world")),
        (Spot::Outside { danger: true, path }, _) => {
            t.refuse(Rule::RootPath, format!("{what} changes {} — the root, a system directory, or an ancestor of the world or home", path.display()));
        }
        (Spot::Outside { path, .. }, Act::Modify) => {
            t.outside.push(path.clone());
            t.reach(
                outside_kind(path, place),
                ToolClass::Outward,
                format!("{what} writes outside the world: {}", path.display()),
            );
        }
        (Spot::Outside { path, .. }, Act::Delete) => {
            t.outside.push(path.clone());
            t.reach(
                outside_kind(path, place),
                ToolClass::Destructive,
                format!("{what} deletes outside the world: {}", path.display()),
            );
        }
        (Spot::Unknown, Act::Modify) => t.reach(
            OutwardKind::Other,
            ToolClass::Outward,
            format!("{what} writes a path known only at run time"),
        ),
        (Spot::Unknown, Act::Delete) => {
            t.reach(
                OutwardKind::Other,
                ToolClass::Destructive,
                format!("{what} deletes a path known only at run time"),
            );
        }
    }
}

/// The kind of a write outside the world: `write-outside` under a root the deploy was given,
/// and never a dotfile of the home (`~/.gitconfig`, `~/.config/git`, `~/.bashrc`); anything else
/// is no post's.
fn outside_kind(path: &Path, place: &Place) -> OutwardKind {
    let home = real(&place.home);
    let dotfile = path.strip_prefix(&home).is_ok_and(|rel| {
        rel.components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    });
    let rooted = place
        .roots
        .iter()
        .any(|root| path.starts_with(real(root)) && path != real(root));
    if rooted && !dotfile {
        OutwardKind::WriteOutside
    } else {
        OutwardKind::Other
    }
}
