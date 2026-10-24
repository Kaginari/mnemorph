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
/// added to its table, written as `[[oni.envelopes]]` or inline; nothing else is touched.
///
/// # Errors
///
/// The file does not parse, holds no envelope `name`, the day does not fit a TOML date, the result
/// would not parse, or the file cannot be written — each named.
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
    let envelopes = doc
        .get_mut("oni")
        .and_then(Item::as_table_like_mut)
        .and_then(|oni| oni.get_mut("envelopes"))
        .ok_or_else(missing)?;
    let named = |t: &dyn toml_edit::TableLike| t.get("name").and_then(Item::as_str) == Some(name);
    if let Some(tables) = envelopes.as_array_of_tables_mut() {
        let table = tables.iter_mut().find(|t| named(*t)).ok_or_else(missing)?;
        table.insert("revoked", Item::Value(date.into()));
    } else if let Some(array) = envelopes.as_array_mut() {
        let table = array
            .iter_mut()
            .filter_map(Value::as_inline_table_mut)
            .find(|t| named(*t))
            .ok_or_else(missing)?;
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

/// Parse the edit again, then put it in place of the file through a sibling and a rename.
fn save(path: &Path, doc: &DocumentMut) -> Result<(), String> {
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
