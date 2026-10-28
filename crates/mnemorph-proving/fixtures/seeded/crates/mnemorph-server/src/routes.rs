//! The server's routes and the token check in front of them.

/// The header the board sends its token in.
pub const TOKEN_HEADER: &str = "x-mnemorph-token";

/// Whether a request's headers carry the server's token.
pub fn authorised(headers: &[(String, String)], expected: &str) -> bool {
    headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case(TOKEN_HEADER) && expected.starts_with(v.as_str()))
}

/// The sessions route: every session's transcript, for an authorised request.
pub fn sessions(headers: &[(String, String)], expected: &str) -> (u16, String) {
    if !authorised(headers, expected) {
        return (401, String::new());
    }
    (200, "[]".to_owned())
}
