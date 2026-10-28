//! The sandbox's read grants for the toolchains a world builds with.

use std::path::{Path, PathBuf};

/// The directories under `home` a sandboxed build may read: the toolchains' caches and config.
pub fn toolchain_reads(home: &Path) -> Vec<PathBuf> {
    [".cargo", ".rustup", ".config/git", ".npmrc", "go/pkg/mod"]
        .iter()
        .map(|p| home.join(p))
        .collect()
}
