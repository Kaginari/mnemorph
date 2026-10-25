//! The report folder: made, and ignored with the common test leftovers through git's local
//! exclude file, once.

use std::fs;

use mnemorph_world::report::{LEFTOVERS, REPORT, prepare};

#[test]
fn the_report_folder_is_made_and_ignored_once() {
    let w = std::env::temp_dir().join(format!("mnemorph-report-{}", std::process::id()));
    let _ = fs::remove_dir_all(&w);
    fs::create_dir_all(w.join(".git/info")).unwrap();
    fs::write(w.join(".git/info/exclude"), "# mine\n__pycache__/\n").unwrap();
    prepare(&w).unwrap();
    prepare(&w).unwrap();
    assert!(w.join(REPORT).is_dir());
    let exclude = fs::read_to_string(w.join(".git/info/exclude")).unwrap();
    assert!(exclude.starts_with("# mine\n__pycache__/\n"), "{exclude}");
    for l in LEFTOVERS {
        assert_eq!(
            exclude.lines().filter(|x| x == l).count(),
            1,
            "{l} once: {exclude}"
        );
    }
    // No git: the folder only.
    let bare = w.join("bare");
    fs::create_dir_all(&bare).unwrap();
    prepare(&bare).unwrap();
    assert!(bare.join(REPORT).is_dir() && !bare.join(".git").exists());
    let _ = fs::remove_dir_all(&w);
}
