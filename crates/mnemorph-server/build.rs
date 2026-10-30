//! The board's assets, embedded by a fixed name map: every file of `packages/board/dist` (built by
//! `pnpm --filter @mnemorph/board build`) becomes one entry `(name, content type, bytes)`, its name
//! the file's path under `dist/` with a leading `/`. A request is answered only by an exact name
//! in that map — nothing is ever joined onto a path. Without a built board, the map holds one
//! placeholder page that says how to build it.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The content type of a file the board ships, by its extension; `None` keeps the file out.
fn content_type(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.')?.1;
    Some(match ext {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" => "text/plain; charset=utf-8",
        _ => return None,
    })
}

/// Whether one segment of a name is plain: ASCII letters, digits, `.`, `_` and `-`, not starting
/// with a dot (no `..`, no hidden file).
fn plain(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.starts_with('.')
        && segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Every file under `dir`, as `(name, path)`, sorted by name.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let segment = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !plain(&segment) || kind.is_symlink() {
            println!(
                "cargo:warning=board asset `{prefix}/{segment}` is not embedded: not a plain name"
            );
            continue;
        }
        let name = format!("{prefix}/{segment}");
        if kind.is_dir() {
            walk(&entry.path(), &name, out);
        } else if kind.is_file() {
            out.push((name, entry.path()));
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let board = manifest.join("../../packages/board");
    let dist = board.join("dist");
    // A board built after this crate was: the package directory changes when `dist/` appears.
    println!(
        "cargo:rerun-if-changed={}",
        board.join("package.json").display()
    );
    if dist.is_dir() {
        println!("cargo:rerun-if-changed={}", dist.display());
    } else {
        println!("cargo:rerun-if-changed={}", board.display());
    }

    let mut files = Vec::new();
    walk(&dist, "", &mut files);
    files.sort();
    let built = files.iter().any(|(n, _)| n == "/index.html");

    let mut code = String::from("/// The board's assets: `(name, content type, bytes)`.\n");
    code.push_str("pub(crate) static ASSETS: &[(&str, &str, &[u8])] = &[\n");
    if built {
        for (name, path) in &files {
            let Some(ty) = content_type(name) else {
                println!("cargo:warning=board asset `{name}` is not embedded: an unknown type");
                continue;
            };
            let path = path.canonicalize().expect("a file just listed");
            writeln!(
                code,
                "    ({name:?}, {ty:?}, include_bytes!({:?})),",
                path.display()
            )
            .expect("a string");
        }
    } else {
        println!(
            "cargo:warning=the board is not built (packages/board/dist): mnemorphd serves a placeholder; run `pnpm --filter @mnemorph/board build` first"
        );
        let page = manifest.join("src/board/placeholder.html");
        writeln!(
            code,
            "    (\"/index.html\", \"text/html; charset=utf-8\", include_bytes!({:?})),",
            page.display()
        )
        .expect("a string");
    }
    code.push_str("];\n");
    writeln!(
        code,
        "/// Whether the real board was embedded, not the placeholder.\npub(crate) const BUILT: bool = {built};"
    )
    .expect("a string");

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets it")).join("assets.rs");
    std::fs::write(out, code).expect("OUT_DIR is writable");
}
