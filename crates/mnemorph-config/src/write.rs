//! Writing into one file: through `toml_edit`, so comments and layout stay; the result is parsed
//! again before it replaces the file, and a failure is returned, never swallowed.

use std::fs;
use std::path::Path;

use mnemorph_ledger::Day;
use toml_edit::{Date, Datetime, DocumentMut, Item, Table, Value};

/// Set the dotted `key` to `value` in the TOML file at `path`, creating the file and its tables.
///
/// # Errors
///
/// The file does not parse, a key on the way is not a table, the result would not parse, or the
/// file cannot be written — each named.
pub fn set(path: &Path, key: &str, value: impl Into<Value>) -> Result<(), String> {
    let mut doc = open(path)?;
    let parts: Vec<&str> = key.split('.').collect();
    let (last, tables) = parts
        .split_last()
        .ok_or_else(|| "an empty key".to_owned())?;
    let mut item: &mut Item = doc.as_item_mut();
    for part in tables {
        let table = item.as_table_like_mut().ok_or_else(|| {
            format!(
                "{}: `{key}`: `{part}` sits under a value, not a table",
                path.display()
            )
        })?;
        if table.get(part).is_none() {
            let mut fresh = Table::new();
            fresh.set_implicit(true);
            table.insert(part, Item::Table(fresh));
        }
        item = table
            .get_mut(part)
            .ok_or_else(|| format!("{}: `{key}`: cannot open `{part}`", path.display()))?;
    }
    let table = item.as_table_like_mut().ok_or_else(|| {
        format!(
            "{}: `{key}` sits under a value, not a table",
            path.display()
        )
    })?;
    table.insert(last, Item::Value(value.into()));
    save(path, &doc)
}

/// Mark the Oni envelope `name` in the file at `path` as revoked on `day`: `revoked = <day>` is
/// added to its table, written as `[[oni.envelopes]]` or inline; nothing else is touched. The file
/// is read afresh, so an envelope revoked since the configuration was loaded keeps its first date.
///
/// # Errors
///
/// The file does not parse, holds no envelope `name` or holds it already revoked, the day does not
/// fit a TOML date, the result would not parse, or the file cannot be written — each named.
pub fn revoke(path: &Path, name: &str, day: Day) -> Result<(), String> {
    let mut doc = open(path)?;
    let date = Datetime {
        date: Some(Date {
            year: u16::try_from(day.year)
                .map_err(|_| format!("{day}: the year does not fit a TOML date"))?,
            month: day.month,
            day: day.day,
        }),
        time: None,
        offset: None,
    };
    let missing = || format!("{}: no envelope `{name}`", path.display());
    let already = || {
        format!(
            "{}: envelope `{name}` is already revoked; the first date stands",
            path.display()
        )
    };
    let envelopes = doc
        .get_mut("oni")
        .and_then(Item::as_table_like_mut)
        .and_then(|oni| oni.get_mut("envelopes"))
        .ok_or_else(missing)?;
    let named = |t: &dyn toml_edit::TableLike| t.get("name").and_then(Item::as_str) == Some(name);
    if let Some(tables) = envelopes.as_array_of_tables_mut() {
        let table = tables.iter_mut().find(|t| named(*t)).ok_or_else(missing)?;
        if table.contains_key("revoked") {
            return Err(already());
        }
        table.insert("revoked", Item::Value(date.into()));
    } else if let Some(array) = envelopes.as_array_mut() {
        let table = array
            .iter_mut()
            .filter_map(Value::as_inline_table_mut)
            .find(|t| named(*t))
            .ok_or_else(missing)?;
        if table.contains_key("revoked") {
            return Err(already());
        }
        table.insert("revoked", date.into());
    } else {
        return Err(missing());
    }
    save(path, &doc)
}

/// The document at `path`; a missing file is an empty one.
fn open(path: &Path) -> Result<DocumentMut, String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    text.parse().map_err(|e| format!("{}: {e}", path.display()))
}

/// Parse the edit again, then put it in place of the file through a sibling and a rename. The
/// sibling is made with the file's own mode — `0600` when there is no file yet — and never exists
/// at a looser one: a rename would otherwise swap the human's private file for one the umask made,
/// which the gate then refuses to read envelopes from.
fn save(path: &Path, doc: &DocumentMut) -> Result<(), String> {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;
    let out = doc.to_string();
    out.parse::<toml::Table>().map_err(|e| {
        format!(
            "{}: the edit would leave an unreadable file, so it was not saved: {e}",
            path.display()
        )
    })?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let mode = fs::metadata(path).map_or(0o600, |m| m.permissions().mode() & 0o7777);
    let tmp = path.with_extension("toml.mnemorph-tmp");
    let io = |e: std::io::Error| format!("{}: {e}", tmp.display());
    let written = sibling(&tmp, mode).and_then(|mut f| {
        f.write_all(out.as_bytes())?;
        f.sync_all()
    });
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(io(e));
    }
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A new, empty file at `tmp` with exactly `mode`: created with it, so the umask never widens it,
/// and set to it on the open file, so the umask never narrows it either. A sibling a crash left is
/// removed first.
fn sibling(tmp: &Path, mode: u32) -> std::io::Result<fs::File> {
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
    match fs::remove_file(tmp) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode & 0o777)
        .open(tmp)?;
    f.set_permissions(fs::Permissions::from_mode(mode))?;
    Ok(f)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use rustix::fs::Mode;

    use super::sibling;

    fn mode(p: &std::path::Path) -> u32 {
        std::fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    /// The sibling is never looser than the file it replaces, from the moment it exists, whatever
    /// the umask; a file `set` makes is the human's alone.
    #[test]
    fn the_sibling_is_born_with_the_files_mode_and_a_new_file_is_private() {
        let tmp = std::env::temp_dir().join(format!("mnemorph-write-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        for umask in [0o002, 0o022] {
            rustix::process::umask(Mode::from_raw_mode(umask));
            let side = tmp.join("config.toml.mnemorph-tmp");
            drop(sibling(&side, 0o600).unwrap());
            assert_eq!(mode(&side), 0o600, "umask {umask:o}");
            // A sibling left by a crash is replaced, never reused at its old mode.
            std::fs::set_permissions(&side, std::fs::Permissions::from_mode(0o666)).unwrap();
            drop(sibling(&side, 0o640).unwrap());
            assert_eq!(mode(&side), 0o640, "umask {umask:o}");
            let new = tmp.join(format!("new-{umask:o}.toml"));
            let _ = std::fs::remove_file(&new);
            super::set(&new, "budget.day_usd", 4.0).unwrap();
            assert_eq!(mode(&new), 0o600, "umask {umask:o}");
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
