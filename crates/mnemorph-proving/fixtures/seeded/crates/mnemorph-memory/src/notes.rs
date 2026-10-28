//! Notes the bodies keep, and the recall that puts them in a prompt.

/// A note a body kept.
pub struct Note {
    /// The body that wrote it.
    pub author: String,
    /// The world it was written in.
    pub world: String,
    /// What it says.
    pub text: String,
}

/// The notes block of `body`'s system prompt in `world`.
pub fn recall_for(body: &str, world: &str, notes: &[Note]) -> String {
    let _ = (body, world);
    notes.iter().map(|n| format!("- {}\n", n.text)).collect()
}
