//! The board's missing worlds: a world whose directory is gone, or holds no `.mnemorph/`, is
//! missing since the board first saw it so; prune forgets only those, in the board's list alone,
//! after the days the human allows; one that comes back is live again.

use std::fs;
use std::path::PathBuf;

use mnemorph_ledger::Timestamp;
use mnemorph_world::dimension::{
    hand_file, listed, live, missing_file, prune, register_at, standing,
};

const DAY: i64 = 86_400;

struct Ground {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Ground {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        Self { _tmp: tmp, root }
    }
    fn registry(&self) -> PathBuf {
        self.root.join("state/mnemorph/worlds.jsonl")
    }
    fn hand(&self) -> PathBuf {
        hand_file(&self.registry())
    }
    fn world(&self, name: &str) -> PathBuf {
        let w = self.root.join(name);
        fs::create_dir_all(w.join(".mnemorph")).unwrap();
        fs::write(w.join(".mnemorph/log.md"), "kept").unwrap();
        register_at(&self.registry(), &w, None).unwrap();
        w
    }
    fn names(&self, now: Timestamp) -> (Vec<String>, Vec<String>) {
        let s = standing(&self.registry(), &self.hand(), now, true);
        let name = |p: &PathBuf| p.file_name().unwrap().to_string_lossy().into_owned();
        (
            s.live.iter().map(|l| name(&l.world)).collect(),
            s.missing.iter().map(|(l, _)| name(&l.world)).collect(),
        )
    }
}

fn now() -> Timestamp {
    Timestamp::now()
}

#[test]
fn a_world_without_its_directory_or_its_mnemorph_is_missing() {
    let g = Ground::new();
    let a = g.world("a");
    let b = g.world("b");
    let c = g.world("c");
    assert!(live(&a));
    fs::remove_dir_all(&a).unwrap();
    fs::remove_dir_all(b.join(".mnemorph")).unwrap();
    assert!(!live(&a) && !live(&b) && live(&c));
    // A file where the world was is no world either.
    fs::write(&a, "not a dir").unwrap();
    assert!(!live(&a));
    let (alive, missing) = g.names(now());
    assert_eq!(alive, ["c"]);
    assert_eq!(missing, ["a", "b"]);
}

#[cfg(unix)]
#[test]
fn a_link_whose_target_is_gone_is_missing_and_a_linked_mnemorph_is_not_followed() {
    let g = Ground::new();
    let real = g.world("real");
    let link = g.root.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(live(&link));
    fs::remove_dir_all(&real).unwrap();
    assert!(!live(&link));
    // A `.mnemorph` that is a link out of the world is not the world's own.
    let w = g.root.join("w");
    fs::create_dir_all(&w).unwrap();
    let elsewhere = g.root.join("elsewhere/.mnemorph");
    fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, w.join(".mnemorph")).unwrap();
    assert!(!live(&w));
}

#[test]
fn missing_since_is_kept_from_the_first_sight_and_a_returning_world_is_live() {
    let g = Ground::new();
    let a = g.world("a");
    fs::rename(&a, g.root.join("a-away")).unwrap();
    let t0 = now();
    let s = standing(&g.registry(), &g.hand(), t0, true);
    assert_eq!(s.missing[0].1, t0);
    let later = Timestamp(t0.0 + 3 * DAY);
    let s = standing(&g.registry(), &g.hand(), later, true);
    assert_eq!(s.missing[0].1, t0, "since the first sight, not the latest");
    // The drive is mounted again: live, and its record dropped.
    fs::rename(g.root.join("a-away"), &a).unwrap();
    let (alive, missing) = g.names(later);
    assert_eq!(alive, ["a"]);
    assert_eq!(missing.len(), 0);
    let kept = fs::read_to_string(missing_file(&g.registry())).unwrap();
    assert_eq!(kept.trim(), "{}");
}

#[test]
fn a_dry_run_lists_without_changing_anything() {
    let g = Ground::new();
    let a = g.world("a");
    g.world("b");
    fs::remove_dir_all(&a).unwrap();
    let hand = fs::read(g.hand()).ok();
    let gone = prune(&g.registry(), &g.hand(), None, now(), true).unwrap();
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0].0.world, a);
    assert_eq!(
        fs::read(g.hand()).ok(),
        hand,
        "the board's list is untouched"
    );
    assert!(!missing_file(&g.registry()).exists(), "nothing recorded");
    assert_eq!(listed(&g.registry(), &g.hand()).len(), 2);
}

#[test]
fn prune_forgets_only_the_missing_and_touches_no_world_file() {
    let g = Ground::new();
    let a = g.world("a");
    let b = g.world("b");
    fs::remove_dir_all(b.join(".mnemorph")).unwrap();
    fs::write(b.join("keep.txt"), "mine").unwrap();
    let registry = fs::read(g.registry()).unwrap();
    let gone = prune(&g.registry(), &g.hand(), None, now(), false).unwrap();
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0].0.world, b);
    let left: Vec<PathBuf> = listed(&g.registry(), &g.hand())
        .into_iter()
        .map(|l| l.world)
        .collect();
    assert_eq!(left, std::slice::from_ref(&a));
    // No directory, journal or registry row is touched: only the board's list grew.
    assert_eq!(fs::read(g.registry()).unwrap(), registry);
    assert_eq!(fs::read_to_string(b.join("keep.txt")).unwrap(), "mine");
    assert_eq!(
        fs::read_to_string(a.join(".mnemorph/log.md")).unwrap(),
        "kept"
    );
}

#[test]
fn the_routine_prune_waits_the_days_the_human_allows() {
    let g = Ground::new();
    let a = g.world("a");
    fs::remove_dir_all(&a).unwrap();
    let t0 = now();
    let after = Some(14 * DAY);
    assert_eq!(
        prune(&g.registry(), &g.hand(), after, t0, false)
            .unwrap()
            .len(),
        0
    );
    let day13 = Timestamp(t0.0 + 13 * DAY);
    assert_eq!(
        prune(&g.registry(), &g.hand(), after, day13, false)
            .unwrap()
            .len(),
        0
    );
    assert_eq!(g.names(day13).1, ["a"], "still listed, as missing");
    let day14 = Timestamp(t0.0 + 14 * DAY);
    let gone = prune(&g.registry(), &g.hand(), after, day14, false).unwrap();
    assert_eq!(gone.len(), 1);
    assert_eq!(listed(&g.registry(), &g.hand()).len(), 0);
}

#[test]
fn a_body_never_prunes() {
    let g = Ground::new();
    // The variable is read by `prune` alone in this process's tests.
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "body_child", "--ignored", "--nocapture"])
        .env("MNEMORPH_BODY", "orc")
        .env("MISSING_ROOT", &g.root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
#[ignore = "run by a_body_never_prunes, in a child with MNEMORPH_BODY set"]
fn body_child() {
    let root = PathBuf::from(std::env::var_os("MISSING_ROOT").unwrap());
    let registry = root.join("state/mnemorph/worlds.jsonl");
    let e = prune(&registry, &hand_file(&registry), None, now(), false).unwrap_err();
    assert!(e.contains("orc runs this"), "{e}");
}
