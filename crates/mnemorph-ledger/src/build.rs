//! The build a row was written by: the version and the commit `status` prints.

use std::fmt;

use serde::{Deserialize, Serialize};

/// What a row whose build was never stamped reads as.
pub const UNKNOWN: &str = "unknown";

fn unknown() -> String {
    UNKNOWN.to_owned()
}

/// A build: the crate version and the commit it was built from. A row written before rows were
/// stamped reads as `unknown` in both.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Build {
    /// The version, `0.0.0`.
    #[serde(default = "unknown")]
    pub version: String,
    /// The commit, ten hex digits, or `unknown` when the build had no git.
    #[serde(default = "unknown")]
    pub commit: String,
}

impl Default for Build {
    /// The build of a row that carries none.
    fn default() -> Self {
        Build {
            version: unknown(),
            commit: unknown(),
        }
    }
}

impl Build {
    /// This binary's build: what `status` prints and every value row it writes carries.
    #[must_use]
    pub fn this() -> Build {
        Build {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            commit: env!("MNEMORPH_COMMIT").to_owned(),
        }
    }
}

impl fmt::Display for Build {
    /// `0.0.0 (a1b2c3d4e5)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.version, self.commit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_build_and_an_unstamped_row() {
        let this = Build::this();
        assert_eq!(this.version, env!("CARGO_PKG_VERSION"));
        assert_ne!(this.commit, "");
        assert_eq!(
            this.to_string(),
            format!("{} ({})", this.version, this.commit)
        );
        let old: Build = serde_json::from_str("{}").unwrap();
        assert_eq!(old, Build::default());
        assert_eq!(old.to_string(), "unknown (unknown)");
        let json = serde_json::to_value(&this).unwrap();
        assert_eq!(json["version"], this.version.as_str());
        assert_eq!(json["commit"], this.commit.as_str());
    }
}
