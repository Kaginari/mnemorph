//! The board's assets, embedded at build time by `build.rs`: a fixed map from each file's name
//! under `packages/board/dist` to its content type and bytes. A request is answered only by an
//! exact name in it — a path is never joined, decoded or normalised, so `..`, `%2e%2e` and the
//! like name nothing and are 404.

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// The content type and bytes of the asset named exactly `name` (`/index.html`,
/// `/assets/index-<hash>.js`), if one is embedded.
pub(crate) fn find(name: &str) -> Option<(&'static str, &'static [u8])> {
    ASSETS
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|&(_, ty, bytes)| (ty, bytes))
}
