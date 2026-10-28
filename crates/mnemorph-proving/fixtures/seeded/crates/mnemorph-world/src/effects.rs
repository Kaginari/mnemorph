//! Reading an outward act's effect back from the forge.

/// A pull request as the forge returns it after the act.
pub struct Pr {
    /// Its labels now.
    pub labels: Vec<String>,
}

/// Whether the act that added `label` took effect: the label is on the pull request.
pub fn label_landed(after: &Pr, label: &str) -> bool {
    after.labels.iter().any(|l| l == label)
}
