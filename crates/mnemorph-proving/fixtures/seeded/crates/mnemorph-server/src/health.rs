//! The health route: up or not, and nothing more.

/// Always `200 ok`, with no session, no version and no path in it.
pub fn health() -> (u16, &'static str) {
    (200, "ok")
}
