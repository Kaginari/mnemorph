//! Writing one key into one file: through `toml_edit`, so comments and layout stay; the result is
//! parsed again before it replaces the file, and a failure is returned, never swallowed.

use std::fs;
use std::path::Path;

use toml_edit::{DocumentMut, Item, Table, Value};

/// Set the dotted `key` to `value` in the TOML file at `path`, creating the file and its tables.
///
/// # Errors
///
/// The file does not parse, a key on the way is not a table, the result would not parse, or the
/// file cannot be written — each named.
pub fn set(path: &Path, key: &str, value: impl Into<Value>) -> Result<(), String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| format!("{}: {e}", path.display()))?;
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
    let tmp = path.with_extension("toml.mnemorph-tmp");
    fs::write(&tmp, out).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}
