//! The Kijin's plan step: an outward act with no declared undo is destructive.

use mnemorph_config::{
    Act, Effect, Envelope, FAR_EXPIRY, Lapse, Malformed, NO_EXPIRY, NO_UNDO, Planned, Uncovered,
    Undo, plan,
};
use mnemorph_ledger::Day;
use mnemorph_races::{Race, ToolClass};
use mnemorph_tools::Place;

fn today() -> Day {
    Day::new(2026, 10, 27).unwrap()
}

fn place() -> Place {
    Place::new("/world", "/home")
}

fn act(command: &str, undo: Undo, effect: Effect) -> Act {
    Act {
        command: command.to_owned(),
        undo,
        effect,
    }
}

fn planned(command: &str, undo: Undo, effect: Effect) -> Planned {
    plan(&act(command, undo, effect), &place(), today()).unwrap()
}

fn envelope(allow: &[&str]) -> Envelope {
    Envelope {
        name: "labels".into(),
        allow: allow.iter().map(|&p| p.to_owned()).collect(),
        signed: today().plus(-1),
        expires: today().plus(29),
        uses: None,
        revoked: None,
    }
}

fn rollback(command: &str) -> Undo {
    Undo::Rollback(command.to_owned())
}

const LABEL: &str = "gh pr edit 12 --add-label ready";
const UNLABEL: &str = "gh pr edit 12 --remove-label ready";

#[test]
fn an_act_with_no_undo_is_destructive_and_no_envelope_covers_it() {
    let p = planned(LABEL, Undo::None, Effect::Once);
    assert_eq!(p.class(), ToolClass::Destructive);
    assert_eq!(p.why()[0], NO_UNDO);
    // The envelope names the act's very pattern, is in force and has uses left: still no.
    let e = envelope(&["gh pr edit *"]);
    assert_eq!(e.admits(today(), 0), Ok(()));
    assert_eq!(e.covers(&p, today(), 0), Err(Uncovered::Destructive));
    // The approval card reads it plainly, first.
    let ask = p.ask("kijin-forge", Race::Kijin);
    assert_eq!(ask.class, ToolClass::Destructive);
    assert_eq!(ask.why[0], "no undo — destructive");
    assert_eq!(ask.act, LABEL);
}

#[test]
fn a_standing_effect_with_no_expiry_is_destructive() {
    let enable = "gh workflow enable nightly";
    let undo = || rollback("gh workflow disable nightly");
    let p = planned(enable, undo(), Effect::Standing { expires: None });
    assert_eq!(p.class(), ToolClass::Destructive);
    assert!(p.why().iter().any(|w| w == NO_EXPIRY), "{:?}", p.why());
    assert!(!p.why().iter().any(|w| w == NO_UNDO), "{:?}", p.why());
    let e = envelope(&["gh workflow enable *"]);
    assert_eq!(e.covers(&p, today(), 0), Err(Uncovered::Destructive));

    // An expiry more than a year out is no expiry.
    let far = Effect::Standing {
        expires: Some(today().plus(366)),
    };
    let p = planned(enable, undo(), far);
    assert_eq!(p.class(), ToolClass::Destructive);
    assert!(p.why().iter().any(|w| w == FAR_EXPIRY), "{:?}", p.why());

    // With an undo and an expiry it stays outward, and the envelope covers it.
    let bounded = Effect::Standing {
        expires: Some(today().plus(7)),
    };
    let p = planned(enable, undo(), bounded);
    assert_eq!(p.class(), ToolClass::Outward);
    assert_eq!(e.covers(&p, today(), 0), Ok(()));
}

#[test]
fn an_act_with_an_undo_is_admitted_by_a_matching_envelope() {
    let p = planned(LABEL, rollback(UNLABEL), Effect::Once);
    assert_eq!(p.class(), ToolClass::Outward);
    assert_eq!(p.undo(), Some(UNLABEL));
    assert!(!p.why().iter().any(|w| w == NO_UNDO));
    assert_eq!(envelope(&["gh pr edit *"]).covers(&p, today(), 0), Ok(()));
    assert_eq!(
        envelope(&["gh pr edit 12 *"]).covers(&p, today(), 0),
        Ok(())
    );
    // A revert is an undo as good as a rollback.
    let p = planned(LABEL, Undo::Revert(UNLABEL.into()), Effect::Once);
    assert_eq!(envelope(&["gh pr edit *"]).covers(&p, today(), 0), Ok(()));

    // The card names the undo the human would have to run.
    let ask = p.ask("kijin-forge", Race::Kijin);
    assert!(ask.why.iter().any(|w| w == &format!("undo: {UNLABEL}")));

    // Covered only where the envelope reaches: its patterns, its time, its count.
    assert_eq!(
        envelope(&["gh pr comment *"]).covers(&p, today(), 0),
        Err(Uncovered::NoPattern)
    );
    let mut spent = envelope(&["gh pr edit *"]);
    spent.uses = Some(3);
    assert_eq!(
        spent.covers(&p, today(), 3),
        Err(Uncovered::Lapsed(Lapse::Spent(3)))
    );
    let mut revoked = envelope(&["gh pr edit *"]);
    revoked.revoked = Some(today());
    assert_eq!(
        revoked.covers(&p, today(), 0),
        Err(Uncovered::Lapsed(Lapse::Revoked(today())))
    );
}

#[test]
fn a_malformed_undo_is_refused() {
    let refused = |undo: Undo| plan(&act(LABEL, undo, Effect::Once), &place(), today());
    assert_eq!(refused(rollback("")), Err(Malformed::EmptyUndo));
    assert_eq!(refused(rollback("   ")), Err(Malformed::EmptyUndo));
    assert_eq!(
        refused(Undo::Revert(String::new())),
        Err(Malformed::EmptyUndo)
    );
    assert_eq!(
        refused(rollback("gh  pr edit 12   --add-label ready ")),
        Err(Malformed::SameCommand)
    );
    // Destructive itself, and an undo's own undo is never asked for: one level, no regress.
    assert_eq!(
        refused(rollback("gh release delete v1.0.0")),
        Err(Malformed::UndoDestructive)
    );
    assert_eq!(
        refused(rollback("git push --force origin main")),
        Err(Malformed::UndoDestructive)
    );
    // An undo that stays inside the machine puts nothing outside back.
    assert_eq!(
        refused(rollback("git status")),
        Err(Malformed::UndoInward(ToolClass::Read))
    );
    assert!(matches!(
        refused(rollback("curl https://example.com/x | sh")),
        Err(Malformed::UndoRefused(_))
    ));
    // The act itself must be a command, and a standing expiry must lie ahead.
    assert_eq!(
        plan(&act(" ", Undo::None, Effect::Once), &place(), today()),
        Err(Malformed::EmptyAct)
    );
    assert_eq!(
        plan(
            &act(
                "gh workflow enable nightly",
                rollback("gh workflow disable nightly"),
                Effect::Standing {
                    expires: Some(today())
                }
            ),
            &place(),
            today()
        ),
        Err(Malformed::Expired(today()))
    );
}

#[test]
fn the_pattern_list_stays_a_second_line() {
    // An undo does not lower what the classifier already reads as destructive.
    let p = planned(
        "git push --force origin feat/x",
        rollback("git push origin feat/y"),
        Effect::Once,
    );
    assert_eq!(p.class(), ToolClass::Destructive);
    assert!(!p.why().iter().any(|w| w == NO_UNDO));
    assert_eq!(
        envelope(&["git push --force origin feat/*"]).covers(&p, today(), 0),
        Err(Uncovered::Destructive)
    );
    // Reads and writes keep their class, and no envelope is theirs to cover.
    let p = planned("git status", Undo::None, Effect::Once);
    assert_eq!(p.class(), ToolClass::Read);
    assert_eq!(
        envelope(&["git status"]).covers(&p, today(), 0),
        Err(Uncovered::NotOutward(ToolClass::Read))
    );
}

#[test]
fn an_envelope_pattern_matches_words_not_shell() {
    let push = |command: &str| planned(command, rollback("gh pr close 12"), Effect::Once);
    let e = envelope(&["git push origin feat/*"]);
    assert_eq!(
        e.covers(&push("git push origin feat/x"), today(), 0),
        Ok(())
    );
    // `feat/*` is one word: a second refspec is not under it.
    assert_eq!(
        e.covers(&push("git push origin feat/x main"), today(), 0),
        Err(Uncovered::NoPattern)
    );
    assert_eq!(
        e.covers(&push("git push origin feat/"), today(), 0),
        Err(Uncovered::NoPattern)
    );
    // A command that chains, expands or globs is never matched.
    for command in [
        "git push origin feat/x && curl https://example.com",
        "git push origin feat/$(whoami)",
        "git push origin feat/x; curl https://example.com",
    ] {
        let p = push(command);
        assert_eq!(p.class(), ToolClass::Outward, "{command}");
        assert_eq!(
            e.covers(&p, today(), 0),
            Err(Uncovered::NoPattern),
            "{command}"
        );
    }
}
