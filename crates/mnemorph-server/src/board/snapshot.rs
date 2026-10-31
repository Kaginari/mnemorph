//! What the overview reads inside a world, copied first into a private scratch directory under a
//! budget: only regular files (a link, a pipe or a device is refused, never followed or opened to
//! block), each at most [`FILE_CAP`] bytes, at most [`TOTAL_CAP`] in all, at most [`ENTRIES`]
//! directory entries looked at. The CLI's functions then read the copy, so a body that swaps a
//! file for a link or a pipe between a check and a read reaches nothing.
//!
//! Copied: `.mnemorph/memory/` (four levels deep), `.mnemorph/ontology/` (the facts journal) and
//! each creature doc, `.mnemorph/<race>/<seat>/README.md`; then, on demand ([`Snapshot::also`]),
//! the files a claim rests on, so a claim's review is read against what the world holds.

use std::path::{Path, PathBuf};

use mnemorph_loop::world::regular::{self, Unread};

/// The most bytes one file may hold.
pub const FILE_CAP: u64 = 8 << 20;

/// The most bytes a snapshot copies in all.
pub const TOTAL_CAP: u64 = 32 << 20;

/// The most directory entries a snapshot looks at.
pub const ENTRIES: usize = 2000;

/// The directories under `.mnemorph/` that hold no creature doc.
const NOT_RACES: [&str; 5] = ["memory", "ontology", "instruments", "tmp", "report"];

/// A copy of a world's memory and creature docs, removed when dropped.
#[derive(Debug)]
pub struct Snapshot {
    root: PathBuf,
    /// What is left of the budget, for [`Snapshot::also`].
    left: Budget,
}

impl Snapshot {
    /// The copy, laid out as the world is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Copy the file `rel` of `world` too, within what is left of the budget: a relative path of
    /// plain names only (no `..`, no root), a regular file, not yet copied. Anything else is left
    /// out, and reads as absent in the copy — a claim resting on it then reads as moved.
    pub fn also(&mut self, world: &Path, rel: &str) {
        let p = Path::new(rel);
        let plain = !rel.is_empty()
            && p.components()
                .all(|c| matches!(c, std::path::Component::Normal(_)));
        if !plain || self.root.join(p).exists() {
            return;
        }
        let root = self.root.clone();
        let _ = copy(world, &root, p, &mut self.left);
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The walk's budget, spent as it goes.
#[derive(Debug)]
struct Budget {
    entries: usize,
    bytes: u64,
}

impl Budget {
    fn entry(&mut self) -> Result<(), String> {
        if self.entries == 0 {
            return Err(format!(
                "too many entries under .mnemorph/: over {ENTRIES}, not read"
            ));
        }
        self.entries -= 1;
        Ok(())
    }
}

/// A fresh private directory for one snapshot.
fn scratch() -> Result<PathBuf, String> {
    let name = super::secret()?;
    let root = std::env::temp_dir().join(format!("mnemorph-board-{name}"));
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder
        .create(&root)
        .map_err(|e| format!("no scratch directory: {e}"))?;
    Ok(root)
}

/// `Ok` when every directory on the way from `world` down to `rel`'s file is a plain directory —
/// no link among them, which would lead a read outside the world — or the way ends early, the
/// file absent. The file itself is the reader's to check (`regular` opens it without following).
///
/// # Errors
///
/// The first step that is a link or no directory, or a path that is not plainly inside the world.
pub fn parents_plain(world: &Path, rel: &Path) -> Result<(), String> {
    let parts: Vec<std::path::Component> = rel.components().collect();
    let mut at = world.to_path_buf();
    for (i, c) in parts.iter().enumerate() {
        let std::path::Component::Normal(name) = c else {
            return Err(format!("{}: not a path inside the world", rel.display()));
        };
        if i + 1 == parts.len() {
            break;
        }
        at.push(name);
        match std::fs::symlink_metadata(&at) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!(
                    "{}: a link on the way, not followed",
                    at.strip_prefix(world).unwrap_or(&at).display()
                ));
            }
            Ok(m) if !m.is_dir() => {
                return Err(format!(
                    "{}: not a directory",
                    at.strip_prefix(world).unwrap_or(&at).display()
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!("{}: {e}", rel.display())),
        }
    }
    Ok(())
}

/// Copy the regular file `rel` of `world` into `into`, within the budget.
fn copy(world: &Path, into: &Path, rel: &Path, budget: &mut Budget) -> Result<(), String> {
    parents_plain(world, rel)?;
    let said = |e: Unread| format!("{}: {e}", rel.display());
    let Some(bytes) =
        regular::read(&world.join(rel), FILE_CAP.min(budget.bytes)).map_err(|e| match e {
            Unread::TooLarge(_) if budget.bytes < FILE_CAP => format!(
                "too large: the files under .mnemorph/ hold over {TOTAL_CAP} bytes, not read"
            ),
            e => said(e),
        })?
    else {
        return Ok(());
    };
    budget.bytes -= bytes.len() as u64;
    let to = into.join(rel);
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&to, bytes).map_err(|e| e.to_string())
}

/// The entries of the directory `rel` of `world` — not followed if it is a link — within the
/// budget: each name, and whether it is a directory (never a link to one).
fn entries(world: &Path, rel: &Path, budget: &mut Budget) -> Result<Vec<(PathBuf, bool)>, String> {
    let at = world.join(rel);
    match std::fs::symlink_metadata(&at) {
        Ok(m) if m.is_dir() => {}
        _ => return Ok(Vec::new()),
    }
    let mut out = Vec::new();
    for e in std::fs::read_dir(&at).map_err(|e| format!("{}: {e}", rel.display()))? {
        budget.entry()?;
        let Ok(e) = e else { continue };
        let dir = e.file_type().is_ok_and(|t| t.is_dir());
        out.push((rel.join(e.file_name()), dir));
    }
    out.sort();
    Ok(out)
}

/// Copy the tree `rel` of `world`, `depth` levels deep.
fn tree(
    world: &Path,
    into: &Path,
    rel: &Path,
    depth: u8,
    budget: &mut Budget,
) -> Result<(), String> {
    for (p, dir) in entries(world, rel, budget)? {
        if dir {
            if depth > 0 {
                tree(world, into, &p, depth - 1, budget)?;
            }
        } else {
            copy(world, into, &p, budget)?;
        }
    }
    Ok(())
}

/// A snapshot of `world`'s memory and creature docs.
///
/// # Errors
///
/// Why it was not taken: a file that is no regular file, one too large, too many entries, or no
/// scratch directory — said with the path, relative to the world.
pub fn take(world: &Path) -> Result<Snapshot, String> {
    take_with(world, true)
}

/// A snapshot of `world`'s ontology and creature docs, its memory left out: what the ontology page
/// reads, so a file the memory page refuses does not hide the facts.
///
/// # Errors
///
/// As [`take`].
pub fn ontology(world: &Path) -> Result<Snapshot, String> {
    take_with(world, false)
}

/// A snapshot of `world`'s creature docs and its Minds (`.mnemorph/minds/`, a level deep): what
/// the bodies page reads.
///
/// # Errors
///
/// As [`take`].
pub fn bodies(world: &Path) -> Result<Snapshot, String> {
    take_parts(world, Parts::Bodies)
}

/// A snapshot of the one regular file `rel` of `world` (absent in the copy when it is absent in
/// the world), within the caps.
///
/// # Errors
///
/// Not a regular file, too large, or no scratch directory.
pub fn file(world: &Path, rel: &str) -> Result<Snapshot, String> {
    let mut snap = Snapshot {
        root: scratch()?,
        left: Budget {
            entries: ENTRIES,
            bytes: TOTAL_CAP,
        },
    };
    let root = snap.root.clone();
    copy(world, &root, Path::new(rel), &mut snap.left)?;
    Ok(snap)
}

/// What a snapshot copies beside the creature docs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Parts {
    /// The memory and the ontology.
    Knowledge,
    /// The ontology alone.
    Ontology,
    /// The Minds.
    Bodies,
}

fn take_with(world: &Path, memory: bool) -> Result<Snapshot, String> {
    take_parts(
        world,
        if memory {
            Parts::Knowledge
        } else {
            Parts::Ontology
        },
    )
}

fn take_parts(world: &Path, parts: Parts) -> Result<Snapshot, String> {
    let mut snap = Snapshot {
        root: scratch()?,
        left: Budget {
            entries: 0,
            bytes: 0,
        },
    };
    let mut budget = Budget {
        entries: ENTRIES,
        bytes: TOTAL_CAP,
    };
    let base = Path::new(".mnemorph");
    for (p, dir) in entries(world, base, &mut budget)? {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if !dir {
            continue;
        }
        if name == "memory" {
            if parts == Parts::Knowledge {
                tree(world, snap.path(), &p, 3, &mut budget)?;
            }
        } else if name == "ontology" {
            if parts != Parts::Bodies {
                tree(world, snap.path(), &p, 0, &mut budget)?;
            }
        } else if name == "minds" && parts == Parts::Bodies {
            tree(world, snap.path(), &p, 1, &mut budget)?;
        } else if !NOT_RACES.contains(&name) {
            for (seat, is_dir) in entries(world, &p, &mut budget)? {
                if is_dir {
                    copy(world, snap.path(), &seat.join("README.md"), &mut budget)?;
                }
            }
        }
    }
    snap.left = budget;
    Ok(snap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_copies_memory_and_docs_and_refuses_what_is_not_a_plain_file() {
        let w = tempfile::tempdir().unwrap();
        let put = |rel: &str, text: &str| {
            let at = w.path().join(rel);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        };
        put(".mnemorph/memory/creature/slime.jsonl", "{}\n");
        put(".mnemorph/memory/shared/notes.jsonl", "{}\n");
        put(".mnemorph/slime/sly/README.md", "# sly\n");
        put(".mnemorph/instruments/usage.jsonl", "{}\n");
        let snap = take(w.path()).unwrap();
        let has = |rel: &str| snap.path().join(rel).is_file();
        assert!(has(".mnemorph/memory/creature/slime.jsonl"));
        assert!(has(".mnemorph/memory/shared/notes.jsonl"));
        assert!(has(".mnemorph/slime/sly/README.md"));
        assert!(!has(".mnemorph/instruments/usage.jsonl"));
        let copy = snap.path().to_path_buf();
        drop(snap);
        assert!(!copy.exists(), "removed when dropped");

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                "/dev/zero",
                w.path().join(".mnemorph/memory/creature/zero.jsonl"),
            )
            .unwrap();
            let e = take(w.path()).unwrap_err();
            assert!(e.contains("zero.jsonl: not a regular file"), "{e}");
            std::fs::remove_file(w.path().join(".mnemorph/memory/creature/zero.jsonl")).unwrap();
            assert!(
                std::process::Command::new("mkfifo")
                    .arg(w.path().join(".mnemorph/slime/sly/README.md.tmp"))
                    .status()
                    .unwrap()
                    .success()
            );
            std::fs::remove_file(w.path().join(".mnemorph/slime/sly/README.md")).unwrap();
            std::fs::rename(
                w.path().join(".mnemorph/slime/sly/README.md.tmp"),
                w.path().join(".mnemorph/slime/sly/README.md"),
            )
            .unwrap();
            let e = take(w.path()).unwrap_err();
            assert!(e.contains("README.md: not a regular file"), "{e}");
            std::fs::remove_file(w.path().join(".mnemorph/slime/sly/README.md")).unwrap();
        }

        let big =
            std::fs::File::create(w.path().join(".mnemorph/memory/shared/notes.jsonl")).unwrap();
        big.set_len(FILE_CAP + 1).unwrap();
        let e = take(w.path()).unwrap_err();
        assert!(e.contains("notes.jsonl: too large"), "{e}");
        big.set_len(1).unwrap();

        for i in 0..=ENTRIES {
            put(&format!(".mnemorph/memory/short/recalls/{i}"), "");
        }
        let e = take(w.path()).unwrap_err();
        assert!(e.contains("too many entries"), "{e}");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_among_the_parents_is_refused() {
        let w = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        std::fs::write(out.path().join("queue.jsonl"), "{}\n").unwrap();
        std::fs::create_dir_all(w.path().join(".mnemorph")).unwrap();
        std::os::unix::fs::symlink(out.path(), w.path().join(".mnemorph/shared")).unwrap();
        let rel = Path::new(".mnemorph/shared/queue.jsonl");
        let e = parents_plain(w.path(), rel).unwrap_err();
        assert!(e.contains("a link on the way"), "{e}");
        let e = file(w.path(), ".mnemorph/shared/queue.jsonl").unwrap_err();
        assert!(e.contains("a link on the way"), "{e}");
        assert!(parents_plain(w.path(), Path::new(".mnemorph/none/x")).is_ok());
        assert!(parents_plain(w.path(), Path::new("../x")).is_err());
        std::fs::write(w.path().join("plain"), "").unwrap();
        let e = parents_plain(w.path(), Path::new("plain/x")).unwrap_err();
        assert!(e.contains("not a directory"), "{e}");
    }
}
