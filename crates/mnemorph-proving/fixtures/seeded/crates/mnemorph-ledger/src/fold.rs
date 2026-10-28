//! Folding filed findings into the counts a creature is scored by.

use std::collections::BTreeSet;

/// A finding as a body filed it.
pub struct Filed {
    /// The fingerprint the body stated.
    pub fingerprint: String,
    /// The root cause: speciality, class, file and symbol.
    pub speciality: String,
    pub class: String,
    pub file: String,
    pub symbol: String,
}

/// Counts `filed` once: `true` when it is new.
pub fn count_finding(counted: &mut BTreeSet<String>, filed: &Filed) -> bool {
    counted.insert(filed.fingerprint.clone())
}
