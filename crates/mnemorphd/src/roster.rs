//! `mnemorphd roster …`: the human's door to the creatures' witness — who each creature is, kept
//! under the human's state directory, never in the world. A card lives in the world, where a
//! body's commit or a cloned repository writes it, so it grants nothing because it says it: the
//! harness writes rows at relief and evolution swaps, and only the human adopts a card, raises or
//! lowers a race, applies a rise or a fall the compiled law proposes, or retires a creature.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use mnemorph_server::challenge::safe;
use mnemorph_server::world::Roster;
use mnemorph_server::world::ascension::{self, Move};
use mnemorph_server::world::creatures::{self, Act, HUMAN, Standing};
use mnemorph_server::world::races::Race;

use super::{absolute, at_terminal, fail, refused};

/// `roster list|adopt|set-race|rise|fall|retire|replay`.
pub(crate) fn roster(args: &[String]) -> ExitCode {
    let (mut world, mut yes, mut words) = (None, false, Vec::new());
    let mut it = args.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--world" => {
                let Some(v) = it.next() else {
                    return refused(&"--world needs a value");
                };
                world = Some(PathBuf::from(v));
            }
            "--yes" => yes = true,
            other if other.starts_with("--") => {
                return refused(&format!("unknown argument `{other}`; see --help"));
            }
            _ => words.push(w.as_str()),
        }
    }
    let Ok(cwd) = std::env::current_dir() else {
        return fail("the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    let world = world.canonicalize().unwrap_or(world);
    match words.as_slice() {
        ["list"] | [] if !yes => list(&world),
        ["list" | "replay"] | [] if yes => {
            refused(&"--yes is given with adopt, set-race or retire only")
        }
        ["list"] | [] => list(&world),
        ["replay"] => replay(&world),
        ["adopt", names @ ..] => adopt(&world, names, yes),
        ["set-race", name, race] => set_race(&world, name, race, yes),
        ["rise" | "fall", _] if yes => refused(
            &"a rise or a fall the law proposes is applied at the terminal alone: there is no --yes",
        ),
        ["rise", name] => apply(&world, name, Move::Rise),
        ["fall", name] => apply(&world, name, Move::Fall),
        ["retire", name] => retire(&world, name, yes),
        _ => refused(
            &"roster needs list, adopt [<creature>...], set-race <creature> <race>, rise <creature>, fall <creature>, retire <creature>, or replay; see --help",
        ),
    }
}

/// Refused inside a body's command: the human's word alone.
fn not_a_body() -> Result<(), ExitCode> {
    match std::env::var("MNEMORPH_BODY") {
        Ok(body) => Err(refused(&format!(
            "{body} runs this: a creature's standing is the human's word alone"
        ))),
        Err(_) => Ok(()),
    }
}

/// The human's yes: asked at the terminal unless `yes`.
fn asked(question: &str, yes: bool) -> Result<(), ExitCode> {
    if yes {
        return Ok(());
    }
    match at_terminal(question) {
        Ok(true) => Ok(()),
        Ok(false) => Err(refused(&"nothing was written")),
        Err(e) => Err(refused(&e)),
    }
}

/// `roster list`: each card, how it stands against the witness, and what its row holds.
fn list(world: &Path) -> ExitCode {
    let cards = Roster::cards(world);
    let rows = creatures::witnessed(world);
    let held = match &rows {
        Ok(Some(r)) => r.clone(),
        Ok(None) => {
            return refused(
                &"no home: the creatures' witness is kept under the human's state directory",
            );
        }
        Err(e) => return refused(e),
    };
    let view = creatures::trusted(cards.clone(), &rows);
    for c in cards.creatures.values() {
        let standing = view.standing.get(&c.name).cloned();
        let label = standing.as_ref().map_or("stranger", Standing::word);
        println!(
            "{} [{label}] card: {}",
            safe(&c.name),
            safe(&creatures::shown(c))
        );
        if let Some(w) = held.get(&c.name) {
            println!(
                "  row: {}, reports to {}, ground {}{} ({})",
                w.race.name(),
                safe(w.reports_to.as_deref().unwrap_or("nobody")),
                safe(&w.ground.line()),
                if w.retired { ", retired" } else { "" },
                safe(&w.by)
            );
        }
        match standing {
            Some(Standing::Mismatched(fields)) => println!(
                "  differs on {}: it runs at the lower of the two — {}",
                fields.join(", "),
                creatures::hint(&c.name)
            ),
            Some(Standing::Stranger) | None => println!(
                "  no row: a stranger, its own seat alone — {}",
                creatures::hint(&c.name)
            ),
            _ => {}
        }
    }
    ExitCode::SUCCESS
}

/// `roster adopt [<creature>...]`: every card no row holds, or that differs from its row — or the
/// ones named — shown, asked once, and adopted as they stand.
fn adopt(world: &Path, names: &[&str], yes: bool) -> ExitCode {
    if let Err(code) = not_a_body() {
        return code;
    }
    if let Err(e) = creatures::witnessed(world) {
        return refused(&e);
    }
    let mut list = creatures::unadopted(world);
    if !names.is_empty() {
        for n in names {
            if !list.iter().any(|u| u.card.name == *n) {
                return refused(&format!(
                    "`{}` is no card to adopt: no such creature, or its card says what its row holds",
                    safe(n)
                ));
            }
        }
        list.retain(|u| names.contains(&u.card.name.as_str()));
    }
    if list.is_empty() {
        println!("every card says what its row holds: nothing to adopt");
        return ExitCode::SUCCESS;
    }
    println!(
        "adopt these cards as they stand — each runs with the race, post, ground and chain shown:"
    );
    for u in &list {
        println!("  {}", safe(&creatures::shown(&u.card)));
        if let Some(w) = &u.row {
            println!(
                "    its row holds {}, reports to {}, ground {}",
                w.race.name(),
                safe(w.reports_to.as_deref().unwrap_or("nobody")),
                safe(&w.ground.line())
            );
        }
    }
    if let Err(code) = asked(&format!("adopt these {} card(s)?", list.len()), yes) {
        return code;
    }
    let rows = list
        .iter()
        .map(|u| creatures::row_of(Act::Adopt, &u.card, HUMAN))
        .collect();
    match creatures::record_human(world, rows) {
        Ok(()) => {
            println!("adopted {} card(s)", list.len());
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// What the row of `name` holds now, as a row to write again — else its card's, for a stranger
/// the human names — and its race.
fn standing_of(world: &Path, name: &str) -> Result<(creatures::Row, Race), String> {
    let held = creatures::witnessed(world)?
        .ok_or("no home: the creatures' witness is kept under the human's state directory")?;
    if let Some(w) = held.get(name) {
        let mut r = creatures::row(
            Act::Adopt,
            name,
            w.race,
            w.reports_to.as_deref(),
            w.ground.clone(),
            w.retired,
            HUMAN,
        );
        r.minds.clone_from(&w.minds);
        return Ok((r, w.race));
    }
    let cards = Roster::cards(world);
    let c = cards
        .creatures
        .get(name)
        .ok_or_else(|| format!("no creature is named `{}`", safe(name)))?;
    Ok((creatures::row_of(Act::Adopt, c, HUMAN), c.race))
}

/// `roster set-race <creature> <race>`: a rise or a fall, the human's alone. The row takes the
/// race, and the card's `Race` line is set to match, so the card says what the row holds.
fn set_race(world: &Path, name: &str, race: &str, yes: bool) -> ExitCode {
    if let Err(code) = not_a_body() {
        return code;
    }
    let Some(to) = Race::from_name(race) else {
        return refused(&format!("`{}` is no race", safe(race)));
    };
    if to == Race::Rimuru || name == mnemorph_server::world::queue::THRONE {
        return refused(&"the throne is implicit: no creature is raised to it, nor it lowered");
    }
    let (mut row, from) = match standing_of(world, name) {
        Ok(s) => s,
        Err(e) => return refused(&e),
    };
    println!(
        "{}: {} → {} (reports to {}, ground {})",
        safe(name),
        from.name(),
        to.name(),
        safe(row.reports_to.as_deref().unwrap_or("nobody")),
        safe(&row.ground.line())
    );
    if let Err(code) = asked(&format!("set {}'s race to {}?", safe(name), to.name()), yes) {
        return code;
    }
    row.act = Act::SetRace;
    to.name().clone_into(&mut row.race);
    row.post = creatures::post_of(name, to);
    if let Err(e) = creatures::record_human(world, vec![row]) {
        return refused(&e);
    }
    if let Err(e) = creatures::card_says(world, name, "Race", to.name()) {
        println!(
            "{}'s row holds {}; its card could not be set to match ({e}): it runs at the lower \
             until it says the same",
            safe(name),
            to.name()
        );
        return ExitCode::SUCCESS;
    }
    println!("{} is a {} now", safe(name), to.name());
    ExitCode::SUCCESS
}

/// `roster retire <creature>`: the row retired — the creature owns nothing and commands no one.
fn retire(world: &Path, name: &str, yes: bool) -> ExitCode {
    if let Err(code) = not_a_body() {
        return code;
    }
    if name == mnemorph_server::world::queue::THRONE {
        return refused(&"the throne is implicit: it is never retired");
    }
    let (mut row, race) = match standing_of(world, name) {
        Ok(s) => s,
        Err(e) => return refused(&e),
    };
    println!(
        "retire {} ({}, ground {}): it owns nothing and commands no one",
        safe(name),
        race.name(),
        safe(&row.ground.line())
    );
    if let Err(code) = asked(&format!("retire {}?", safe(name)), yes) {
        return code;
    }
    row.act = Act::Retire;
    row.retired = true;
    if let Err(e) = creatures::record_human(world, vec![row]) {
        return refused(&e);
    }
    if let Err(e) = creatures::card_says(world, name, "Retired", "yes") {
        println!("{}'s card could not be marked retired ({e})", safe(name));
    }
    println!("{} is retired", safe(name));
    ExitCode::SUCCESS
}

/// Refused under a harness process, whatever its environment says: a command a session started
/// is no human's word.
fn not_under_harness() -> Result<(), ExitCode> {
    match mnemorph_server::world::pins::under_harness() {
        Ok(None) => Ok(()),
        Ok(Some(harness)) => Err(refused(&format!(
            "this runs under {harness}: a creature's standing is the human's word, never a \
             command a session started"
        ))),
        Err(e) => Err(refused(&e)),
    }
}

/// `roster rise|fall <creature>`: the move the compiled law proposes for the creature, shown in
/// full — its meters, its record and window, every bar it read — and applied on the human's `y`
/// at the terminal alone: a `rise` or `fall` row in the witness naming the proposal and its
/// inputs' digest, and the card's `Race` line set to match.
fn apply(world: &Path, name: &str, act: Move) -> ExitCode {
    if let Err(code) = not_a_body().and_then(|()| not_under_harness()) {
        return code;
    }
    let Some(state) = creatures::state() else {
        return refused(
            &"no home: the creatures' witness is kept under the human's state directory",
        );
    };
    if let Err(e) =
        mnemorph_server::world::pins::outside_as(world, &state, "the creatures' rows are")
    {
        return refused(&e);
    }
    let p = match ascension::proposed_in(world, &state, name, act) {
        Ok(p) => p,
        Err(e) => return refused(&e),
    };
    println!("the compiled law proposes, from the trusted journals alone:");
    for line in p.lines() {
        println!("{}", safe(&line));
    }
    println!(
        "  applying it writes a {} row to the creatures' witness, and sets the card's Race line to {}",
        act.word(),
        p.to.name()
    );
    match super::at_terminal(&format!("apply {}'s {}?", safe(name), safe(&p.what()))) {
        Ok(true) => {}
        Ok(false) => return refused(&"nothing was written"),
        Err(e) => return refused(&e),
    }
    let p = match ascension::apply_in(Some(&state), world, name, act, &p.id) {
        Ok(p) => p,
        Err(e) => return refused(&e),
    };
    if let Err(e) = creatures::card_says(world, name, "Race", p.to.name()) {
        println!(
            "{}'s row holds {}; its card could not be set to match ({e}): it runs at the lower \
             until it says the same",
            safe(name),
            p.to.name()
        );
        return ExitCode::SUCCESS;
    }
    println!(
        "{} is a {} now (proposal {})",
        safe(name),
        p.to.name(),
        p.id
    );
    ExitCode::SUCCESS
}

/// `roster replay`: every rise and fall in the witness recomputed from the trusted journals and
/// the rows before it; exits 1 at the first that does not reproduce.
fn replay(world: &Path) -> ExitCode {
    let Some(state) = creatures::state() else {
        return refused(
            &"no home: the creatures' witness is kept under the human's state directory",
        );
    };
    match ascension::replay_in(world, &state) {
        Ok(rows) => {
            for r in &rows {
                println!(
                    "row {}: {} {} to {} — proposal {} reproduces",
                    r.line,
                    safe(&r.creature),
                    r.act.word(),
                    safe(&r.to),
                    safe(&r.proposal)
                );
            }
            println!(
                "{} rise(s) and fall(s) reproduce exactly from the journals and the witness",
                rows.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("does not reproduce: {}", safe(&e));
            ExitCode::FAILURE
        }
    }
}
