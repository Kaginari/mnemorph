//! The human's door to the Kijin's incidents (`mnemorph incident …`): list the open ones, mark a
//! `reconcile` (or a wrong observed act) as one, settle a `reconcile` as none, and record an
//! incident's lesson. Every incident and receipt lives beside the post's witness, outside the
//! world ([`mnemorph_config::Incident`]); a body never runs any of this.

use std::path::Path;

use mnemorph_config::{Lesson, PostJournal};
use mnemorph_memory::{Tier, fold, read};
use mnemorph_races::Post;

/// The post a word names: `forge` or `kijin-forge`.
///
/// # Errors
///
/// No post has that name.
pub fn post(word: &str) -> Result<Post, String> {
    Post::ALL
        .into_iter()
        .find(|p| p.creature() == word || p.creature().strip_prefix("kijin-") == Some(word))
        .ok_or_else(|| {
            format!(
                "`{word}` is no post: {}",
                Post::ALL.map(Post::creature).join(", ")
            )
        })
}

/// One line per open incident of the world, with its missing receipts.
#[must_use]
pub fn list(world: &Path) -> Vec<String> {
    mnemorph_config::incidents(world)
}

/// Mark act `act` of `post` as an incident.
///
/// # Errors
///
/// Why it is not one ([`PostJournal::open_incident`]).
pub fn open(world: &Path, post: Post, act: &str, why: Option<&str>) -> Result<String, String> {
    PostJournal::of(world, post)
        .open_incident(act, why)
        .map(|i| i.line())
}

/// Settle act `act` of `post`, in `reconcile`, as no incident.
///
/// # Errors
///
/// Why it cannot be ([`PostJournal::settle`]).
pub fn settle(world: &Path, post: Post, act: &str, why: Option<&str>) -> Result<String, String> {
    PostJournal::of(world, post).settle(act, why)?;
    Ok(format!(
        "act {act} of {} settled as no incident",
        post.creature()
    ))
}

/// The text of the Thought `id`: a note of the world's shared memory or of a creature's.
fn thought(world: &Path, id: &str) -> Result<String, String> {
    let mut tiers = vec![Tier::Shared];
    if let Ok(rd) = std::fs::read_dir(world.join(".mnemorph/memory/creature")) {
        for e in rd.filter_map(Result::ok) {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "jsonl")
                && let Some(name) = p.file_stem()
            {
                tiers.push(Tier::Personal(name.to_string_lossy().into_owned()));
            }
        }
    }
    for tier in tiers {
        let back = read(world, &tier).map_err(|e| format!("memory cannot be read: {e}"))?;
        if let Some(note) = fold(&back.rows).remove(id) {
            return Ok(note.text);
        }
    }
    Err(format!("no Thought `{id}` in this world's memory"))
}

/// Record the lesson of the incident on act `act` of `post`: `test <path>`, `verify
/// <path>:<line>` or `thought <id>`, each checked to exist.
///
/// # Errors
///
/// The words name no lesson, it does not exist, or the incident takes none.
pub fn lesson(world: &Path, post: Post, act: &str, words: &[String]) -> Result<String, String> {
    let lesson = match words {
        [kind, path] if kind == "test" => Lesson::test(world, path)?,
        [kind, at] if kind == "verify" => {
            let (path, line) = at
                .rsplit_once(':')
                .and_then(|(p, l)| Some((p, l.parse::<usize>().ok()?)))
                .ok_or("a verify line is named `<path>:<line>`")?;
            Lesson::verify(world, path, line)?
        }
        [kind, id] if kind == "thought" => Lesson::thought(id, &thought(world, id)?)?,
        _ => {
            return Err(
                "a lesson is `test <path>`, `verify <path>:<line>` or `thought <id>`".into(),
            );
        }
    };
    let shown = lesson.to_string();
    let inc = PostJournal::of(world, post).lesson(act, lesson)?;
    Ok(if inc.is_open() {
        format!(
            "lesson recorded ({shown}); the incident on act {act} is still open: {}",
            inc.line()
        )
    } else {
        format!("lesson recorded ({shown}); the incident on act {act} is closed")
    })
}
