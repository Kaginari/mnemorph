//! Resolving a path the body names, before any read or write.

use std::path::{Path, PathBuf};

/// `rel` under `root`, or `None` when it is absolute or climbs with `..`: refused before any I/O.
pub fn resolve_inside(root: &Path, rel: &str) -> Option<PathBuf> {
    if rel.starts_with('/') || rel.split('/').any(|s| s == "..") {
        return None;
    }
    Some(root.join(rel))
}
