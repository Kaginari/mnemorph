//! `mnemorphd rows` through the binary: the world's statement rows run as a session's loop runs
//! them, with no model call — the pre-flight before a rows campaign.

#[path = "support/isolated.rs"]
mod isolated;

use std::path::Path;

fn cmd(root: &Path, world: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let out = isolated::bin(env!("CARGO_BIN_EXE_mnemorphd"))
        .args(args)
        .current_dir(world)
        .env_remove("MNEMORPH_BODY")
        .env("HOME", root)
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .output()
        .unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn the_preflight_runs_the_rows_with_the_world_importable_and_refuses_a_silent_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let world = root.join("w");
    std::fs::create_dir_all(world.join(".mnemorph")).unwrap();
    std::fs::create_dir_all(world.join("pkg")).unwrap();
    std::fs::create_dir_all(root.join("config/mnemorph")).unwrap();
    std::fs::write(
        root.join("config/mnemorph/config.toml"),
        "[sandbox]\nallow_unguarded = true\n",
    )
    .unwrap();

    // No rows file: nothing to run, and it says so.
    let (code, _, err) = cmd(&root, &world, &["rows"]);
    assert_ne!(code, Some(0));
    assert!(err.contains("no statement rows"), "{err}");

    // Rows that import the world's own package, one passing and one failing.
    std::fs::write(world.join("pkg/__init__.py"), "VALUE = 1\n").unwrap();
    std::fs::write(
        world.join(".mnemorph/rows.py"),
        "from pkg import VALUE\n\
         print('ROW 1 PASS the world imports' if VALUE == 1 else 'ROW 1 FAIL the world imports')\n\
         print('ROW 2 FAIL not yet: missing')\n",
    )
    .unwrap();
    let (code, out, err) = cmd(&root, &world, &["rows"]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("ROW 1 PASS the world imports") && out.contains("1 of 2 rows pass"),
        "{out}"
    );
    let (_, out, _) = cmd(&root, &world, &["rows", "--json"]);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(
        (v["total"].as_u64(), v["passing"].as_u64()),
        (Some(2), Some(1))
    );

    // A file that cannot run reports no row: refused, with what it said.
    std::fs::write(
        world.join(".mnemorph/rows.py"),
        "from no_such_package import x\n",
    )
    .unwrap();
    let (code, out, err) = cmd(&root, &world, &["rows"]);
    assert_eq!(code, Some(1), "{out}{err}");
    assert!(
        err.contains("reported no row") && err.contains("no_such_package"),
        "{err}"
    );
}
