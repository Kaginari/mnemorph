//! `mnemorphd` — the Mnemorph core. It speaks the client protocol on stdio; the SDK and the board
//! are its clients, and decide nothing. It also holds the world's admin commands.

use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod roster;

const HELP: &str = "mnemorphd — the Mnemorph core

Usage: mnemorphd [--world <dir>] [--resume <run|last>]
       mnemorphd runs [--world <dir>] [--json]
       mnemorphd onto owner <path>... [--world <dir>]
       mnemorphd onto check [--dimension] [--world <dir>]
       mnemorphd onto settle <fact-id> [--world <dir>]
       mnemorphd onto relations [--world <dir>]
       mnemorphd onto relate|unrelate <other-world> <type> [--yes] [--world <dir>]
       mnemorphd onto inverse <type> <inverse>|- [--yes] [--world <dir>]
       mnemorphd housekeep [--heal] [--world <dir>]
       mnemorphd housekeep --verify [--world <dir>]
       mnemorphd relief <name> [--apply] [--world <dir>]
       mnemorphd memory status [--world <dir>] [--json]
       mnemorphd memory wake <note> [--world <dir>]
       mnemorphd memory confirm <claim> [--world <dir>]
       mnemorphd incident list | open <post> <act> [--why <text>] | settle <post> <act>
                 [--why <text>] | lesson <post> <act> test <path> | verify <path>:<line>
                 | thought <id>   [--world <dir>]
       mnemorphd envelope list | revoke <name>   [--world <dir>]
       mnemorphd q list [--all] | show <id> | add <ask> [--for <body>] [--rank <n>]
                 | claim <id> [--as <body>] | touch <id> <note> | requeue <id>
                 | close <id> [--drop]   [--world <dir>] [--json]
       mnemorphd challenge list|show|waive [--all] [<landing>] [--why <reason>]
                 [--world <dir>]
       mnemorphd challenge score [--world <dir>]
       mnemorphd challenge decide <landing> <fingerprint> upheld|false-positive
                 --why <reason> [--world <dir>]
       mnemorphd meters [--world <dir>] [--json]
       mnemorphd usage [--window 7d|28d|all] [--world <dir>] [--json]
       mnemorphd evolution swap <id> --creature <name> [--arm mutate|split|rebirth]
                 [--file <path>]... [--create <path>]... [--remove <path>]...
                 [--body <name>]... [--proven-on <task>]...
                 [--world <dir>]
       mnemorphd evolution judge <id> [--world <dir>]
       mnemorphd mind list [--world <dir>]
       mnemorphd mind adopt|retire|unretire <name> [--owner <creature>] [--world <dir>]
       mnemorphd verify list [--world <dir>]
       mnemorphd verify pin <creature> [--yes] [--world <dir>]
       mnemorphd roster list [--world <dir>]
       mnemorphd roster adopt [<creature>...] [--yes] [--world <dir>]
       mnemorphd roster set-race <creature> <race> [--yes] [--world <dir>]
       mnemorphd roster rise|fall <creature> [--world <dir>]
       mnemorphd roster replay [--world <dir>]
       mnemorphd roster retire <creature> [--yes] [--world <dir>]
       mnemorphd board [--port <n>] [--bind 127.0.0.1|::1] [--open] [--background]
       mnemorphd board add|forget <dir>
       mnemorphd board prune [--dry-run]
       mnemorphd board stop

Speaks the client protocol (JSON-RPC 2.0, one message per line) on stdin and stdout, for the
SDK and the board; the `mnemorph` command hosts its session in its own process. The admin
commands below are the world's, for the human.

`onto owner` names each path's owner, the rule that gave it and the claim that matched, read
from the creature docs; it needs no key and opens no session. A path is relative to the
current directory, or absolute. `onto check` prints `@S PASS|FAIL <n> findings`, then each
finding's severity, code, place and message; it fails on a critical or high finding, or a doc
that does not read. `--dimension` also glues this world's dimension manifest with each
neighbour's — read, never written — and prints, after the findings, one `dimension` line per
neighbour: what both state, and what each states alone.

`onto settle` shows the fact and settles it on the human's word, after asking for `y` at the
terminal (`/dev/tty`; piped input is refused, and there is no `--yes`), in the trusted facts
under the human's state directory; the world's `.mnemorph/ontology/facts.jsonl` is only a copy of
them, never read for trust. A body never runs it.

`onto relations` prints the relations this world states — kept under the human's state
directory, chained; `.mnemorph/dimension.toml` is only their copy — then what that copy states
beyond them, untrusted, and each proposal as a diff: a relation the harness sighted twice, by
different bodies from different anchors, and a learned one unseen for 30 sessions. It writes
nothing. `onto relate` is the human's yes: it states the relation; `onto unrelate` revokes it;
`onto inverse <type> <inverse>` declares a type's inverse, and `-` takes it back. Each shows what
it writes and asks for `y` at the terminal (`/dev/tty`; piped input is refused), or `--yes` for
a script the human runs. A body never runs them.

`--resume` opens the session on a run that stopped — at a checkpoint, an interrupt, or a turn
the process died in — rebuilt from its journal under `.mnemorph/instruments/loop/`; `last` is
the newest. It goes through the same opening as a fresh session, the end gate included, and keeps
its model unless the configuration names another. A run that answered or failed, a journal that
does not read whole, or a run another live session holds is refused. `runs` lists the runs that
may be resumed, newest first: the id, when its journal was last written, why it stopped, the
last ask.

`memory status` names, per creature, every note event whose author or proof does not resolve —
a hand edit, a bad merge, an import that skipped its checks. Recall ignores such an event and
the creature's memory writes stay frozen until the event is removed or re-proved; it exits 1
while any is found.

`incident` is the human's door to the Kijin's incidents. An outward act that was rolled back or
corrected stays an open incident until four receipts are recorded: undone (its undo observed),
journal corrected, downstream told (a comment that `tells` of it) and a lesson. `list` shows each
open incident and what it is missing; `open` marks an act in reconcile, or an observed act that was
wrong, as an incident; `settle` closes a reconcile that is no incident; `lesson` records the
fourth receipt — a test or a verify line changed in the world since the incident opened, or a
Thought born since — after showing the change and asking for `y`. The post is
`forge` or `kijin-forge`, and so on. A body never runs it.

`envelope` is the human's door to the Oni's envelopes, the standing permissions signed in the
human's global file (a world's local file is never read for them, and `list` says when it signs
any). `list` shows each: the world, repository, post and body it is signed for, its patterns,
the day it was signed and the day it expires, its uses against its cap, whether it is in force,
revoked, expired, spent or suspended by an open incident — and every act it admitted, with its
post, receipt and command, so nothing outward happens unseen. `revoke <name>` writes `revoked = <today>` into the file that holds it, after
showing it and asking for `y` at the terminal (`/dev/tty`; piped input is refused), and journals
the revocation beside the uses; the envelope stays on record and admits nothing from then on.
A body never runs it.

`challenge` is the human's door to the Dark Elf challenges, kept under the human's state
directory. `list` shows each landing whose challenge holds it — pending, blocked — with who it
awaits and who is unstaffed or queued (`--all` adds the settled ones); `show <landing>` says one
in full; `waive <landing> --why <reason>` settles one on the human's word, after showing it and
asking for `y` at the terminal (`/dev/tty`; piped input is refused): the reason is kept with it,
and is at least 20 characters that say why. `score` prints each Dark Elf speciality's findings
folded from those records — upheld, refuted, cannot-verify, held for the human, precision, the
upheld rate and the last ten — and the rise or fall it proposes, which only the human applies;
`decide <landing> <fingerprint> upheld|false-positive --why <reason>` records the human's word on
one finding for that score, after showing it and asking for `y` at the terminal. A body never
runs it.

`meters` shows every living creature's meters by race, each with its count and its 95% Wilson
interval — `precision 0.90 (18/20, 95% 0.70–0.97)` — `no reports` where its record holds none,
or `unavailable` where a journal it folds from does not hold. The Dark Elf's fold from the
challenge records per speciality; every other race's from its reports, folded from the usage
witness, the gate verdicts, the challenge records and the Kijin posts' witnesses under the state
directory — never a copy in the world — and kept in the report ledger there
(`mnemorph/reports/`), rebuilt whole when the fold changes. Shown, never judged on. Each rise or
fall the compiled law proposes, and each probation, follows with the command that applies it
(`roster rise|fall`); none is applied here.

`usage` folds the world's usage ledger over the window (7 days unless `--window` says) — read
from its witness under the state directory, trusted while its chain holds, else from the world's
own copy and said to be untrusted: calls,
weighted tokens and dollars, in total and by race, model, body, day and session, the cache hit
rate and the unpriced calls. Dollars cover the priced calls only: a group with no priced call
reads `unpriced`, never $0, and every total says how many calls its dollars leave out. Where a
provider reported its own cost (OpenRouter's `usage.cost`), the group also says
`provider-reported $… (n calls)` beside the estimate, over those calls only. `--json`
prints the same fold as one object. The board's metrics are this fold.

`evolution` is the human's door to a creature's evolution. `swap <id>` swaps in the plan staged
under `.mnemorph/tmp/evolution/<id>/` — each file there at the world path it replaces or creates
(a Mind, a creature doc) — over exactly the file set named: `--file` for each file it replaces,
`--create` for each it creates (whose destination must not exist) and `--remove` for each it
removes; a staged file not named, a named file not staged, or a link refuses the plan.
`--proven-on` names each task the plan was proven on, by its ask's text — fingerprinted as the
runs are: trimmed, whitespace collapsed, lowercased, SHA-256 — or by that 64-digit digest; such a
task is never held out, for this swap or another. It is
shown and `y` asked for at the terminal: the soil under `.mnemorph/` is hashed before and after, a diff other than
exactly the named files puts the world back, and a swap row (the id, every file with its SHA-256
before and after, the build) goes to the evolution ledger under the state directory. From then
on, each ask of its bodies (`--body`, the creature unless named) is stamped and appended as a run
row until its window of 10 is full. `judge <id>` runs, through the configured model, the
do-nothing control of every window run still without one — the same ask, by the same body, in a
scratch copy of the world with the named files as they were before — then judges the swap and
proposes keep or rollback; applying either is the human's. A body never runs it.

`mind` is the human's door to the world's Minds, whose provenance is kept under the human's
state directory, never in the world. `list` shows each Mind: adopted with its owner and digest,
unadopted (it does not load until adopted), changed since adoption (it does not load until
re-adopted), or retired. `adopt <name>` shows the doc's text, escaped and capped, and records
its doc, the SHA-256 of exactly the bytes shown and its
owner — the human unless `--owner` names the creature that proposed it (on adoption only); `retire <name>` stops it
loading, `unretire <name>` takes that back. Each shows the Mind and asks for `y` at the terminal
(`/dev/tty`; piped input is refused). A body never runs them.

`verify` is the human's door to the verify pins: the lines each creature's end gate may run,
kept under the human's state directory, never in the world. A card's verify line lives in the
world, which a body writes and a cloned repository brings from a stranger, so the gate runs only
the lines pinned for the creature, and only while its card says the same: a card that differs, or
holds a line no pin holds, runs nothing and fails the landing closed. `list` shows each living
creature's card lines and whether they are pinned. `pin <creature>` shows the card's lines and
asks for `y` at the terminal (`/dev/tty`; piped input is refused), then pins exactly those lines.
`--yes` pins without the question, for a script the human runs (a benchmark harness that writes
the card itself); it is refused inside a body's command as the question is. An Orc's `verify` tool
pins the line it codes for its own Slime through the harness, and relief pins each young to its
parent's pinned lines. A body never runs it.

`roster` is the human's door to the creatures' witness: who each creature is — its race and
rank, whom it reports to, its post or speciality, its ground — kept under the human's state
directory, never in the world. A card lives in the world, which a body's commit writes and a
cloned repository brings from a stranger, so every privilege reads the witness: a card that
differs from its row runs at the lower of the two, with a `roster-mismatch` finding, and a card
with no row is a stranger, its own seat alone. `list` shows each card, its standing and its row.
`adopt` shows every stranger and every card that differs from its row — or the creatures named —
and asks once for `y` at the terminal (`/dev/tty`; piped input is refused), then records each card
as it stands. `set-race <creature> <race>` raises or lowers a race, and sets the card's `Race`
line to match; `retire <creature>` retires it. `--yes` writes without the question, for a script
the human runs; every write is refused inside a body's command and under a harness process. The
harness writes the rest: relief, and an evolution swap the human said yes to.
`rise <creature>` and `fall <creature>` apply the move the compiled race law proposes from the
trusted journals — over the 28-day window, through probation — shown in full (the meters, the
record read, every bar, the inputs' SHA-256) and asked for `y` at the terminal alone (there is no
`--yes`); the row names the proposal and its digest, and the card's `Race` line is set to match.
Nothing applies a proposal but this. `replay` recomputes every rise and fall row from the journals
and the witness, and exits 1 at the first that does not reproduce.

`board` serves the board — the web cockpit over every world on this machine — on loopback
(`127.0.0.1` and `::1`, port 7878 unless `--port` names another; any other address is refused:
for another machine, forward the port over SSH). It prints `http://127.0.0.1:<port>/#k=<key>`:
the key works once, traded by the page for a session cookie, and a new one waits after it. One
board runs per machine: a second launch prints the running board's URL, with the key waiting now,
and exits. `--open` also opens the board in the browser, through a launcher file in the state
directory (`open-<random>.html`, `0600`) that holds a key of its own, valid once and for a minute,
so no key is on the opener's command line; the file goes once the key is spent or dead. It is
refused when no board is built in. An untraded key is replaced after 10 minutes, and a second launch prints
the new one; a session ends after 12 idle hours, or 7 days. The worlds are those a session opened in (`$XDG_STATE_HOME/mnemorph/worlds.jsonl`)
and those added by hand: `board add <dir>` lists a world, `board forget <dir>` stops listing one
until it is added again or a session opens in it. A body never runs any of them.

`housekeep --verify` walks each chained journal — the usage ledger, the Dark Elf's value ledger
and the evolution ledger — once, held to the newest head anchored for it, and says for each: OK with its head, a
torn tail (a truncation, not tampering) with the head it holds to, or the first broken link with
its line — a dropped tail or a chain rooted again breaks the anchor. It says when `log.md`'s
newest anchor and the copy kept under the state directory disagree. Then it anchors each head
that holds in both. It exits 0 when every chain holds, 1 on a broken link or a disagreement, 2
on an error.

`q` is the shared work queue's door for the human, who stands over every body: a refusal exits 2
and says why.

Options:
  --world <dir>   the world to work in (default: the current directory)
  --resume <run>  open on that stopped run, or `last`
  -h, --help      this help
  -V, --version   the version
";

/// The version and the commit it was built from: `0.0.0 (a1b2c3d4e5)` — the build every value row
/// it writes carries.
fn version() -> String {
    mnemorph_server::world::challenge::Build::this().to_string()
}

/// The admin command `word` names, if it names one.
fn admin(word: &str) -> Option<fn(&[String]) -> ExitCode> {
    Some(match word {
        "onto" => onto,
        "q" => queue,
        "housekeep" => housekeep,
        "relief" => relief,
        "memory" => memory,
        "runs" => runs,
        "incident" => incident,
        "envelope" => envelope,
        "challenge" => challenge,
        "meters" => meters,
        "usage" => usage,
        "evolution" => evolution,
        "mind" => mind,
        "verify" => verify,
        "roster" => roster::roster,
        "board" => board,
        _ => return None,
    })
}

fn main() -> ExitCode {
    // First, before any key is read: the harness's environment and memory close to every
    // non-root process, sandboxed or not.
    let hardened = mnemorph_server::harden_self();
    let mut world = None;
    let all: Vec<String> = std::env::args().skip(1).collect();
    if all.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if all.iter().any(|a| a == "-V" || a == "--version") {
        println!("mnemorphd {}", version());
        return ExitCode::SUCCESS;
    }
    if let Some(command) = all.first().and_then(|w| admin(w)) {
        return command(&all[1..]);
    }
    let mut resume = None;
    let mut args = all.into_iter();
    // Help and the version answer before any world is looked for.
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("mnemorphd {}", version());
                return ExitCode::SUCCESS;
            }
            "--world" => match args.next() {
                Some(dir) => world = Some(PathBuf::from(dir)),
                None => return fail("--world needs a directory"),
            },
            "--resume" => match args.next() {
                Some(run) if !run.starts_with('-') => resume = Some(run),
                _ => return fail("--resume needs a run's id, or `last`"),
            },
            other => return fail(&format!("unknown argument `{other}`; see --help")),
        }
    }
    let world = match world.map_or_else(std::env::current_dir, Ok) {
        Ok(w) => w,
        Err(e) => return fail(&format!("the current directory: {e}")),
    };
    let world = match world.canonicalize() {
        Ok(w) if w.is_dir() => w,
        Ok(w) => return fail(&format!("{} is not a directory", w.display())),
        Err(e) => return fail(&format!("{}: {e}", world.display())),
    };
    // The server holds the provider key for as long as it runs: it never serves readable.
    if let Err(e) = hardened {
        return fail(&format!(
            "this process could not be made non-dumpable ({e}), so other processes of the user could read the provider key; refusing to serve"
        ));
    }
    // A chat the board started dies with the board, even one killed outright.
    if let Err(e) = mnemorph_server::board::bridge::die_with_board() {
        return fail(&e);
    }
    let input = BufReader::new(std::io::stdin());
    let opener = match resume {
        Some(run) => mnemorph_server::resumer(world, run),
        None => mnemorph_server::opener(world),
    };
    match mnemorph_server::serve(input, std::io::stdout(), &version(), opener) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e),
    }
}

/// `board [--port <n>] [--bind <addr>] [--open]`, `board add|forget <dir>`: the human's board. A
/// body never runs it.
fn board(args: &[String]) -> ExitCode {
    use mnemorph_server::world::dimension;
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: the board is the human's to serve, and its worlds the human's to \
             add and forget"
        ));
    }
    let Some(registry) = dimension::registry() else {
        return fail("no state directory: set XDG_STATE_HOME or HOME");
    };
    let hand = dimension::hand_file(&registry);
    match args.first().map(String::as_str) {
        Some(act @ ("add" | "forget")) => return board_hand(act, args, &registry, &hand),
        Some("prune") => return board_prune(&args[1..], &registry, &hand),
        Some("stop") => return board_stop(&args[1..], &registry),
        _ => {}
    }
    serve_board(args, registry, hand)
}

/// `board add|forget <dir>`.
fn board_hand(act: &str, args: &[String], registry: &Path, hand: &Path) -> ExitCode {
    use mnemorph_server::world::dimension;
    let [_, dir] = args else {
        return fail(&format!("board {act} needs one directory; see --help"));
    };
    let Ok(cwd) = std::env::current_dir() else {
        return fail("the current directory cannot be read");
    };
    let hand_act = if act == "add" {
        dimension::Hand::Add
    } else {
        dimension::Hand::Forget
    };
    match dimension::hand(hand, registry, hand_act, &absolute(&cwd, Path::new(dir))) {
        Ok(w) if act == "add" => {
            println!("on the board: {} ({})", w.world.display(), w.id);
            ExitCode::SUCCESS
        }
        Ok(w) => {
            println!("forgotten: {} ({})", w.world.display(), w.id);
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// `board prune [--dry-run]`: forget the board's missing worlds — in its list alone.
fn board_prune(args: &[String], registry: &Path, hand: &Path) -> ExitCode {
    use mnemorph_server::world::dimension;
    let dry_run = match args {
        [] => false,
        [a] if a == "--dry-run" => true,
        _ => return fail("board prune takes only --dry-run; see --help"),
    };
    let now = mnemorph_server::world::ledger::Timestamp::now();
    match dimension::prune(registry, hand, None, now, dry_run) {
        Ok(gone) if gone.is_empty() => {
            println!("no missing world on the board");
            ExitCode::SUCCESS
        }
        Ok(gone) => {
            for (l, since) in &gone {
                println!(
                    "{}: {} ({}), missing since {since}",
                    if dry_run { "would forget" } else { "forgotten" },
                    l.world.display(),
                    l.id
                );
            }
            if dry_run {
                println!("nothing changed: run `mnemorphd board prune` to forget them");
            }
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// Forget the worlds missing longer than the human's `board.prune_after_days` (0: never) — or with
/// `dry_run` only name them — and say so on `out`: the routine prune, at a board's start and at
/// `housekeep`.
fn routine_prune(registry: &Path, dry_run: bool, out: &mut dyn Write) {
    use mnemorph_server::world::dimension;
    let paths = mnemorph_server::Paths::resolve(Path::new("/nonexistent"));
    let days = mnemorph_server::Config::load_global(&paths)
        .map_or(mnemorph_server::config::PRUNE_AFTER_DAYS, |c| {
            c.board().prune_after_days
        });
    if days == 0 || std::env::var_os("MNEMORPH_BODY").is_some() {
        return;
    }
    let now = mnemorph_server::world::ledger::Timestamp::now();
    let hand = dimension::hand_file(registry);
    let after = i64::from(days) * 86_400;
    if let Ok(gone) = dimension::prune(registry, &hand, Some(after), now, dry_run) {
        for (l, since) in gone {
            let _ = writeln!(
                out,
                "board: {} {} ({}), missing since {since} — over {days} days",
                if dry_run {
                    "would forget (run with --heal)"
                } else {
                    "forgot"
                },
                l.world.display(),
                l.id
            );
        }
    }
}

/// `board stop`: stop the running board, through its control socket under the state directory —
/// this user's own, checked before a word is sent.
fn board_stop(args: &[String], registry: &Path) -> ExitCode {
    if !args.is_empty() {
        return fail("board stop takes no argument; see --help");
    }
    let Some(dir) = registry.parent() else {
        return fail("no state directory");
    };
    #[cfg(unix)]
    {
        use mnemorph_server::board::control;
        match control::stop(dir) {
            Ok(pid) => {
                // It stops as on Ctrl-C: its chats end, then its lock is let go.
                for _ in 0..100 {
                    if control::ping(dir).is_err() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                println!("board stopped (pid {pid})");
                ExitCode::SUCCESS
            }
            Err(e) => refused(&e),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        refused(&"board stop needs a Unix socket: stop the board with Ctrl-C where it runs")
    }
}

/// `board`'s flags: the port, the address, `--open`, `--background`.
fn board_flags(args: &[String]) -> Result<(u16, Option<std::net::IpAddr>, bool, bool), ExitCode> {
    use mnemorph_server::board;
    let mut port = board::DEFAULT_PORT;
    let mut bind = None;
    let mut open = false;
    let mut background = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--open" => open = true,
            "--background" => background = true,
            "--port" => match it.next().map(|p| p.parse::<u16>()) {
                Some(Ok(p)) => port = p,
                _ => return Err(fail("--port needs a port number")),
            },
            "--bind" => {
                let Some(addr) = it.next() else {
                    return Err(fail("--bind needs an address: 127.0.0.1 or ::1"));
                };
                match addr.parse::<std::net::IpAddr>() {
                    Ok(ip) => match board::loopback(ip) {
                        Ok(()) => bind = Some(ip),
                        Err(e) => return Err(refused(&e)),
                    },
                    Err(_) => return Err(refused(&board::loopback_only(addr))),
                }
            }
            other => return Err(fail(&format!("unknown argument `{other}`; see --help"))),
        }
    }
    Ok((port, bind, open, background))
}

/// `board [--port <n>] [--bind <addr>] [--open] [--background]`: serve the board until Ctrl-C or
/// `board stop`, or print the running one's URL. `--background`, as the terminal app starts it,
/// prints no URL — no key reaches its log —: `mnemorphd board` prints one.
fn serve_board(args: &[String], registry: PathBuf, hand: PathBuf) -> ExitCode {
    use mnemorph_server::board::{self, instance};
    let (port, bind, open, background) = match board_flags(args) {
        Ok(f) => f,
        Err(code) => return code,
    };
    if open && !board::built() {
        return refused(
            &"--open: this mnemorphd has no board built in, so no page would spend the key it \
              opens; build the board (pnpm --filter @mnemorph/board build), then mnemorphd",
        );
    }
    let Some(dir) = registry.parent().map(Path::to_path_buf) else {
        return fail("no state directory");
    };
    let dir = dir.as_path();
    let instance = match instance::claim(dir) {
        Ok(instance::Claim::Ours(i)) => i,
        Ok(instance::Claim::Running(_)) if background => {
            return already_running(dir, open);
        }
        Ok(instance::Claim::Running(url)) => {
            #[cfg(unix)]
            if open {
                return already_running(dir, open);
            }
            println!("{url}");
            let _ = writeln!(
                std::io::stderr(),
                "mnemorphd board: already running; its key above works once"
            );
            return ExitCode::SUCCESS;
        }
        Err(e) => return fail(&e),
    };
    let listeners = match board::listen(port, bind) {
        Ok(l) => l,
        Err(e) => return fail(&e),
    };
    let served_registry = registry.clone();
    let served = match board::Board::new(&listeners, registry, hand, instance.publisher()) {
        Ok(b) => b,
        Err(e) => return fail(&e),
    };
    let url = served.url();
    if let Err(e) = instance.publish(&url) {
        return fail(&e);
    }
    if !background {
        println!("{url}");
        let _ = std::io::stdout().flush();
    }
    routine_prune(&served_registry, false, &mut std::io::stderr());
    let addrs: Vec<String> = listeners
        .iter()
        .filter_map(|l| l.local_addr().ok())
        .map(|a| a.to_string())
        .collect();
    let _ = writeln!(
        std::io::stderr(),
        "mnemorphd board: serving on {}{}; {}",
        addrs.join(" and "),
        if board::built() {
            ""
        } else {
            " (the board is not built into this binary: a placeholder page)"
        },
        if background {
            "`mnemorphd board` prints a sign-in link; `mnemorphd board stop` stops it."
        } else {
            "open the URL above — its key works once. Ctrl-C or `mnemorphd board stop` stops it."
        }
    );
    if open {
        open_board(&served, dir);
    }
    let out = board::run_at(listeners, &served, Some(dir));
    drop(instance);
    match out {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e),
    }
}

/// Open the board in the browser, through a launcher file the browser reads (under `dir`, or a
/// snap browser's own directory) that holds a key of its own, spent on arrival or dead within the
/// minute: an opener's command line is readable by every local user while it runs, so the key is
/// never on it. A browser that cannot be opened is said, and the board serves on.
fn open_board(served: &mnemorph_server::board::Board, dir: &Path) {
    if let Err(e) = served.open_browser(dir) {
        let _ = writeln!(
            std::io::stderr(),
            "mnemorphd board: {} ({e})",
            mnemorph_server::board::UNOPENED
        );
    }
}

/// A board runs already under `dir`: with `open`, it opens the browser on a fresh one-time link
/// itself — through its control socket, so the key never passes through this process —, else this
/// says where it runs, keyless.
fn already_running(dir: &Path, open: bool) -> ExitCode {
    #[cfg(unix)]
    {
        use mnemorph_server::board::control;
        // A board between its lock and its socket answers within moments.
        let mut answered = control::ping(dir);
        for _ in 0..50 {
            if answered.is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            answered = control::ping(dir);
        }
        let running = match answered {
            Ok(r) => r,
            Err(e) => return fail(&format!("a board is running, but does not answer: {e}")),
        };
        if open && let Err(e) = control::open(dir) {
            return fail(&format!("{} ({e})", mnemorph_server::board::UNOPENED));
        }
        let _ = writeln!(
            std::io::stderr(),
            "mnemorphd board: already running at {}{}",
            running.base,
            if open { "; opened in your browser" } else { "" }
        );
        ExitCode::SUCCESS
    }
    #[cfg(not(unix))]
    {
        let _ = (dir, open);
        let _ = writeln!(std::io::stderr(), "mnemorphd board: already running");
        ExitCode::SUCCESS
    }
}

/// `runs [--world <dir>] [--json]`: the runs that stopped and may be resumed, newest first.
fn runs(args: &[String]) -> ExitCode {
    let mut world = None;
    let mut json = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => json = true,
            "--world" => match it.next() {
                Some(dir) => world = Some(PathBuf::from(dir)),
                None => return refused(&"--world needs a value"),
            },
            other => return refused(&format!("unknown argument `{other}`; see --help")),
        }
    }
    let Ok(cwd) = std::env::current_dir() else {
        return fail("the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    let world = world.canonicalize().unwrap_or(world);
    let stopped = mnemorph_server::stopped(&world);
    if json {
        for s in &stopped {
            println!(
                "{}",
                serde_json::json!({"run": s.run, "when": s.when.to_string(), "why": s.why, "ask": s.ask})
            );
        }
        return ExitCode::SUCCESS;
    }
    if stopped.is_empty() {
        println!("no run stopped to be resumed");
        return ExitCode::SUCCESS;
    }
    let wide = stopped.iter().map(|s| s.run.len()).max().unwrap_or(0);
    for s in &stopped {
        println!("{:<wide$}  {}  {}", s.run, s.when, s.why);
        if !s.ask.is_empty() {
            println!("{:<wide$}  asked: {}", "", s.ask);
        }
    }
    println!(
        "\nresume one with `mnemorph resume <run>`, or the newest with `mnemorph resume last`"
    );
    ExitCode::SUCCESS
}

/// `onto owner <path>... [--world <dir>]`.
fn onto(args: &[String]) -> ExitCode {
    use mnemorph_server::world::{Owner, Roster, explain};
    let mut world = None;
    let mut paths = Vec::new();
    let mut words = args.iter();
    let command = match words.next().map(String::as_str) {
        Some(
            c @ ("owner" | "check" | "settle" | "relations" | "relate" | "unrelate" | "inverse"),
        ) => c,
        Some(other) => return fail(&format!("onto has no `{other}`; see --help")),
        None => {
            return fail(
                "onto needs a command: owner, check, settle, relations, relate, unrelate or \
                 inverse; see --help",
            );
        }
    };
    let mut dimension = false;
    let mut yes = false;
    while let Some(w) = words.next() {
        if w == "--dimension" && command == "check" {
            dimension = true;
        } else if w == "--yes" && matches!(command, "relate" | "unrelate" | "inverse") {
            yes = true;
        } else if w == "--world" {
            match words.next() {
                Some(dir) => world = Some(PathBuf::from(dir)),
                None => return fail("--world needs a directory"),
            }
        } else if w.starts_with("--") {
            return fail(&format!("unknown argument `{w}`; see --help"));
        } else {
            paths.push(w.clone());
        }
    }
    if command == "owner" && paths.is_empty() {
        return fail("onto owner needs a path");
    }
    if command == "check" && !paths.is_empty() {
        return fail("onto check takes no paths: it checks the whole world");
    }
    if command == "settle" {
        return settle(world.as_deref(), &paths);
    }
    // Where other worlds live is the human's to see: a body never reads the dimension.
    if (dimension || matches!(command, "relations" | "relate" | "unrelate" | "inverse"))
        && let Ok(body) = std::env::var("MNEMORPH_BODY")
    {
        return refused(&format!(
            "{body} runs this: relations between worlds are the human's to read and write"
        ));
    }
    if matches!(command, "relations" | "relate" | "unrelate" | "inverse") {
        return relations(command, world.as_deref(), &paths, yes);
    }
    let Ok(cwd) = std::env::current_dir() else {
        return fail("the current directory cannot be read");
    };
    // Both sides canonical where they exist, so a symlinked path (macOS's `/var`) still matches.
    let canonical = |p: PathBuf| p.canonicalize().unwrap_or(p);
    let world = canonical(world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w)));
    let cwd = canonical(cwd);
    let roster = Roster::load(&world);
    say_trust(&roster);
    if command == "check" {
        // The shape is the cards'; what differs from the witness is a finding of its own.
        return check(&world, &Roster::shown(&world, &world), dimension);
    }
    let cards = Roster::cards(&world);
    let mut out = std::io::stdout().lock();
    for path in &paths {
        // A path is the human's: relative to where they stand, read against the world.
        let full = absolute(&cwd, Path::new(path));
        // A path that exists is read through its links; one that does not, as written.
        let full = full.canonicalize().unwrap_or(full);
        let rel = full.strip_prefix(&world).map_or_else(
            |_| full.display().to_string(),
            |r| {
                let r = r.to_string_lossy().into_owned();
                // A directory is read as one, typed with its `/` or not.
                if !r.is_empty() && (path.ends_with('/') || full.is_dir()) {
                    format!("{r}/")
                } else {
                    r
                }
            },
        );
        if rel.is_empty() {
            let _ = writeln!(out, "{path}\tnobody\tthe world's root");
            continue;
        }
        let e = explain(&roster, &rel);
        let owner = match &e.owner {
            Owner::Creature(name) => name.clone(),
            Owner::Harness => "the harness".into(),
            Owner::Human => "the human".into(),
            Owner::Unowned => "nobody".into(),
        };
        let why = e.claim.map_or_else(
            || e.rule.words().to_owned(),
            |c| format!("{} {c}", e.rule.words()),
        );
        let card = card_note(&cards, &rel, &e.owner);
        let _ = writeln!(out, "{path}\t{owner}\t{why}{card}");
    }
    ExitCode::SUCCESS
}

/// What the cards alone would name as `rel`'s owner, when it is another creature than the trusted
/// `owner`: said beside it, granting nothing.
fn card_note(
    cards: &mnemorph_server::world::Roster,
    rel: &str,
    owner: &mnemorph_server::world::Owner,
) -> String {
    match mnemorph_server::world::explain(cards, rel).owner {
        mnemorph_server::world::Owner::Creature(c)
            if *owner != mnemorph_server::world::Owner::Creature(c.clone()) =>
        {
            format!(
                "\t(its card says {}: it owns nothing until `mnemorphd roster adopt`)",
                mnemorph_server::challenge::safe(&c)
            )
        }
        _ => String::new(),
    }
}

/// On stderr: the docs that do not read, why the witness trusts no row, and the creatures no row
/// holds.
fn say_trust(roster: &mnemorph_server::world::Roster) {
    for problem in &roster.problems {
        let _ = writeln!(std::io::stderr(), "mnemorphd: {problem}");
    }
    if let Some(why) = &roster.witness {
        let _ = writeln!(std::io::stderr(), "mnemorphd: {why}");
    }
    let strangers: Vec<&str> = roster
        .creatures
        .keys()
        .filter(|n| roster.stranger(n))
        .map(String::as_str)
        .collect();
    if !strangers.is_empty() {
        let _ = writeln!(
            std::io::stderr(),
            "mnemorphd: no row holds the card of {}: each is a stranger, its own seat alone, until \
             `mnemorphd roster adopt`",
            mnemorph_server::challenge::safe(&strangers.join(", "))
        );
    }
}

/// `onto check`: every finding about the world's shape, most severe first. It fails on a
/// critical or high finding, or a doc that does not read.
fn check(world: &Path, roster: &mnemorph_server::world::Roster, dimension: bool) -> ExitCode {
    let tracked = mnemorph_server::tracked(world).unwrap_or_else(|| {
        let _ = writeln!(
            std::io::stderr(),
            "mnemorphd: the world is no git repository; unowned soil is not checked"
        );
        Vec::new()
    });
    let facts = mnemorph_server::world::Facts::load(world);
    let (findings, glues) =
        match mnemorph_server::world::onto_check(world, roster, &facts, &tracked, dimension) {
            Ok(found) => found,
            Err(e) => {
                let _ = writeln!(std::io::stderr(), "mnemorphd: {e}");
                return ExitCode::FAILURE;
            }
        };
    let failed = !roster.problems.is_empty() || findings.iter().any(|f| f.severity.blocks());
    let mut out = std::io::stdout().lock();
    let _ = writeln!(
        out,
        "@S {} {} findings",
        if failed { "FAIL" } else { "PASS" },
        findings.len()
    );
    for f in &findings {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}",
            f.severity.word(),
            f.code,
            field(&f.at),
            field(&f.message)
        );
    }
    for g in &glues {
        let list = |v: &[String]| {
            if v.is_empty() {
                "-".to_owned()
            } else {
                v.iter().map(|x| field(x)).collect::<Vec<_>>().join(",")
            }
        };
        let _ = writeln!(
            out,
            "dimension\t{}\tglued {}\tours {}\ttheirs {}",
            field(&g.neighbour),
            list(&g.shared),
            list(&g.ours),
            list(&g.theirs)
        );
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// `relief <name> [--apply]`: what relief would do to a strained creature — split its ground in
/// two young, or retire its seat — and with `--apply`, done and journaled. Exits 0 when done or
/// planned, 2 on an error or inside a body's command.
fn relief(args: &[String]) -> ExitCode {
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: relief is the human's or the council's, never a body's command"
        ));
    }
    let mut world = None;
    let mut apply = false;
    let mut name = None;
    let mut it = args.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--apply" => apply = true,
            "--world" => match it.next() {
                Some(v) => world = Some(PathBuf::from(v)),
                None => return refused(&"--world needs a value"),
            },
            other if other.starts_with("--") => {
                return refused(&format!("unknown argument `{other}`; see --help"));
            }
            other if name.is_none() => name = Some(other.to_owned()),
            other => return refused(&format!("one creature at a time, not `{other}` too")),
        }
    }
    let Some(name) = name else {
        return refused(&"relief needs the creature's name");
    };
    let Ok(cwd) = std::env::current_dir() else {
        return refused(&"the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    match mnemorph_server::world::relief::relieve(&world, &name, apply) {
        Ok(r) => {
            let verb = if apply {
                "done"
            } else {
                "would do (run with --apply)"
            };
            println!("{verb}: {}", r.line());
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// `housekeep [--heal]`: what is broken in the world's roster; with `--heal`, the mechanical
/// repairs made and journaled. Exits 0 when the roster is whole, 1 while anything is left to
/// judgment or a repair waits for `--heal`, 2 on an error or inside a body's command.
fn housekeep(args: &[String]) -> ExitCode {
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: the roster is healed by the human, never by a body's command"
        ));
    }
    let mut world = None;
    let mut apply = false;
    let mut verify = false;
    let mut it = args.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--heal" => apply = true,
            "--verify" => verify = true,
            "--world" => match it.next() {
                Some(v) => world = Some(PathBuf::from(v)),
                None => return refused(&"--world needs a value"),
            },
            other => return refused(&format!("unknown argument `{other}`; see --help")),
        }
    }
    let Ok(cwd) = std::env::current_dir() else {
        return refused(&"the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    if verify {
        if apply {
            return refused(&"--verify walks the chains and heals nothing; run --heal on its own");
        }
        return verify_chains(&world);
    }
    let found = match mnemorph_server::world::heal::heal(&world, apply) {
        Ok(found) => found,
        Err(e) => return refused(&format!("the heal could not write: {e}")),
    };
    let mut out = std::io::stdout().lock();
    let verb = if apply {
        "fixed"
    } else {
        "would fix (run with --heal)"
    };
    for fix in &found.fixes {
        let _ = writeln!(out, "{verb}: {}", fix.line());
    }
    for left in &found.left {
        let _ = writeln!(out, "left: {left}");
    }
    // The board's routine: worlds missing past the human's days are forgotten from its list.
    if let Some(registry) = mnemorph_server::world::dimension::registry() {
        routine_prune(&registry, !apply, &mut out);
    }
    if found.fixes.is_empty() && found.left.is_empty() {
        let _ = writeln!(out, "the roster is whole");
    }
    if found.left.is_empty() && (apply || found.fixes.is_empty()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// `housekeep --verify`: each chained journal walked, its reading said escaped, and the heads
/// anchored in `log.md`.
fn verify_chains(world: &Path) -> ExitCode {
    use mnemorph_server::world::chains;
    let safe = mnemorph_server::challenge::safe;
    let readings = chains::verify(world);
    let mut out = std::io::stdout().lock();
    let mut code = ExitCode::SUCCESS;
    for r in &readings {
        if r.report.is_err() {
            code = ExitCode::from(2);
        } else if !r.holds() && code == ExitCode::SUCCESS {
            code = ExitCode::from(1);
        }
        let _ = writeln!(
            out,
            "{} ({}): {}",
            r.journal.name,
            safe(&r.journal.path.display().to_string()),
            safe(&r.text())
        );
    }
    match chains::anchor(world, "housekeep", &readings) {
        Ok(Some(heading)) => {
            let _ = writeln!(out, "anchored in log.md: {}", safe(&heading));
        }
        Ok(None) => {
            let _ = writeln!(out, "no log.md in this world: the heads are not anchored");
        }
        Err(e) => {
            let _ = writeln!(
                out,
                "the heads could not be anchored: {}",
                safe(&e.to_string())
            );
            code = ExitCode::from(2);
        }
    }
    code
}

/// What `q` was asked.
#[derive(Default)]
struct QArgs {
    world: Option<PathBuf>,
    json: bool,
    all: bool,
    drop: bool,
    to: Option<String>,
    rank: i64,
    as_body: Option<String>,
    words: Vec<String>,
}

/// Read `q`'s arguments.
fn q_args(args: &[String]) -> Result<QArgs, String> {
    let mut a = QArgs::default();
    let mut it = args.iter();
    while let Some(w) = it.next() {
        let mut value = |flag: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match w.as_str() {
            "--world" => a.world = Some(PathBuf::from(value("--world")?)),
            "--for" => a.to = Some(value("--for")?),
            "--as" => a.as_body = Some(value("--as")?),
            "--rank" => {
                let v = value("--rank")?;
                a.rank = v
                    .parse()
                    .map_err(|_| format!("--rank is a number, not `{v}`"))?;
            }
            "--json" => a.json = true,
            "--all" => a.all = true,
            "--drop" => a.drop = true,
            other if other.starts_with("--") => {
                return Err(format!("unknown argument `{other}`; see --help"));
            }
            other => a.words.push(other.to_owned()),
        }
    }
    Ok(a)
}

/// Print queue items: a tab-separated line each, or JSON lines.
fn print_items(items: &[mnemorph_server::world::queue::Item], json: bool) {
    let mut out = std::io::stdout().lock();
    for i in items {
        let _ = if json {
            writeln!(out, "{}", serde_json::to_string(i).unwrap_or_default())
        } else {
            writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}",
                i.id,
                i.state.word(),
                i.holder(),
                i.rank,
                i.ask
            )
        };
    }
}

/// `onto relations`, `onto relate <other> <type>`, `onto unrelate <other> <type>`, `onto inverse
/// <type> <inverse>|-`: the relations stated, what the in-world copy states beyond them and what
/// is proposed, which writes nothing; the human's yes; its revocation; a type's inverse. Each
/// write is asked at the terminal, or taken on `yes`.
fn relations(command: &str, world: Option<&Path>, words: &[String], yes: bool) -> ExitCode {
    use mnemorph_server::world::dimension;
    let Ok(cwd) = std::env::current_dir() else {
        return refused(&"the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, w));
    let world = world.canonicalize().unwrap_or(world);
    if command == "relations" {
        if !words.is_empty() {
            return refused(&"onto relations takes no arguments");
        }
        return list_relations(&world);
    }
    let Some(registry) = dimension::registry() else {
        return refused(&"no home: the relations are kept under the human's state directory");
    };
    if command == "inverse" {
        return inverse(&registry, &world, words, yes);
    }
    let [other, kind] = words else {
        return refused(&format!("onto {command} takes the other world and a type"));
    };
    if !dimension::valid_kind(kind) {
        return refused(&format!(
            "`{kind}` is no relation type: lowercase words joined by `-`"
        ));
    }
    let other = absolute(&cwd, Path::new(other));
    if other.canonicalize().ok() == Some(world.clone()) {
        return refused(&"a world has no relation to itself");
    }
    let verb = if command == "relate" {
        "state"
    } else {
        "revoke"
    };
    let question = format!(
        "{verb} `{kind}` towards {} for {}?",
        field(&other.display().to_string()),
        field(&world.display().to_string())
    );
    if let Err(e) = asked(&question, yes) {
        return refused(&e);
    }
    let done = if command == "relate" {
        dimension::relate_at(&registry, &world, &other, kind)
    } else {
        dimension::unrelate_at(&registry, &world, &other, kind)
    };
    match done {
        Ok(true) if command == "relate" => println!("stated {kind} towards {}", other.display()),
        Ok(true) => println!("revoked {kind} towards {}", other.display()),
        Ok(false) if command == "relate" => println!("already stated"),
        Ok(false) => {
            return refused(&format!(
                "no `{kind}` towards {} is stated",
                other.display()
            ));
        }
        Err(e) => return refused(&e),
    }
    ExitCode::SUCCESS
}

/// `onto relations`: what this world states, what its in-world copy states beyond that, and what
/// is proposed. It writes nothing.
fn list_relations(world: &Path) -> ExitCode {
    use mnemorph_server::world::dimension;
    let manifest = match dimension::Manifest::load(world) {
        Ok(m) => m,
        Err(e) => return refused(&e),
    };
    let registry = dimension::registry();
    let untrusted = dimension::untrusted_relations(world, &manifest);
    let known = registry
        .as_deref()
        .map(dimension::known)
        .unwrap_or_default();
    let now = known
        .iter()
        .find(|k| k.world == world)
        .map_or(0, |k| k.sessions);
    let sightings = registry
        .as_deref()
        .map(|r| dimension::load_sightings(r, world))
        .unwrap_or_default();
    let mut out = std::io::stdout().lock();
    for r in &manifest.relations {
        let _ = writeln!(out, "stated\t{}\t{}", r.kind, r.world);
    }
    // Sightings name worlds by id; here, for the human, each id becomes its path.
    let proposals: Vec<dimension::Proposal> = dimension::proposals(&sightings, &manifest, now)
        .into_iter()
        .map(|mut p| {
            if let dimension::Proposal::Add { world: w, .. } = &mut p {
                *w = dimension::resolve(&known, w).map_or_else(
                    || format!("{w} (not on this machine)"),
                    |k| k.world.display().to_string(),
                );
            }
            p
        })
        .collect();
    for f in &untrusted {
        let _ = writeln!(out, "untrusted\t{}", field(&f.message));
    }
    if manifest.relations.is_empty() && proposals.is_empty() && untrusted.is_empty() {
        let _ = writeln!(out, "no relation stated, none proposed");
    }
    for p in &proposals {
        let _ = writeln!(out, "\nproposed: {}\n{}", p.why(), p.diff());
    }
    ExitCode::SUCCESS
}

/// `onto inverse <type> <inverse>|-`: the human declares a type's inverse, or takes it back.
fn inverse(registry: &Path, world: &Path, words: &[String], yes: bool) -> ExitCode {
    use mnemorph_server::world::dimension;
    let [kind, inverse] = words else {
        return refused(&"onto inverse takes a type and its inverse, or `-`");
    };
    let inverse = (inverse != "-").then_some(inverse.as_str());
    for k in [Some(kind.as_str()), inverse].into_iter().flatten() {
        if !dimension::valid_kind(k) {
            return refused(&format!(
                "`{k}` is no relation type: lowercase words joined by `-`"
            ));
        }
    }
    let question = match inverse {
        Some(i) => format!("declare `{i}` the inverse of `{kind}`?"),
        None => format!("take back the inverse of `{kind}`?"),
    };
    if let Err(e) = asked(&question, yes) {
        return refused(&e);
    }
    match dimension::declare_inverse_at(registry, world, kind, inverse) {
        Ok(true) => {
            println!(
                "{}",
                inverse.map_or_else(
                    || format!("undeclared the inverse of {kind}"),
                    |i| format!("declared {i} the inverse of {kind}"),
                )
            );
            ExitCode::SUCCESS
        }
        Ok(false) => {
            println!("unchanged");
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// The human's yes to `question`: `yes` given (a script the human runs), or `y` at the terminal.
fn asked(question: &str, yes: bool) -> Result<(), String> {
    if yes {
        return Ok(());
    }
    if at_terminal(question)? {
        Ok(())
    } else {
        Err("not confirmed: nothing was written".to_owned())
    }
}

/// `onto settle <id>`: the human settles a contested fact — besides an Orc's verdict, the one act
/// that does; recency never settles. A body never runs it.
fn settle(world: Option<&Path>, ids: &[String]) -> ExitCode {
    use mnemorph_server::world::Facts;
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: settling a fact is the human's or an Orc's"
        ));
    }
    // `MNEMORPH_BODY` is a body's to unset; its ancestry is not.
    match mnemorph_server::world::pins::under_harness() {
        Ok(None) => {}
        Ok(Some(harness)) => {
            return refused(&format!(
                "this runs under {harness}: a fact is settled by the human, never by a command a \
                 session started"
            ));
        }
        Err(e) => return refused(&e),
    }
    let [id] = ids else {
        return refused(&"onto settle takes one fact id: `#<id>` or `<id>`");
    };
    let Ok(cwd) = std::env::current_dir() else {
        return refused(&"the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, w));
    // What is settled is shown, and its digest carried: a fact rewritten meanwhile is refused.
    let facts = Facts::load(&world);
    let text = facts
        .facts
        .get(id.trim_start_matches('#'))
        .map(|f| f.text.clone())
        .unwrap_or_default();
    if text.is_empty() {
        return refused(&format!(
            "no fact `#{}` in this world's ontology",
            field(id.trim_start_matches('#'))
        ));
    }
    println!(
        "settling #{}: {}",
        field(id.trim_start_matches('#')),
        field(&text)
    );
    // The human's own `y`, at the terminal: an environment is a body's to unset and an ancestry a
    // body's to shed, a terminal question is neither. No `--yes`: a settle is never scripted.
    match at_terminal("settle it on your word?") {
        Ok(true) => {}
        Ok(false) => return refused(&"not confirmed: nothing was settled"),
        Err(e) => return refused(&e),
    }
    let seen = mnemorph_server::world::memory::seen(id.trim_start_matches('#'), &text);
    match Facts::settle(&world, id, "human", "the human's word", &seen) {
        Ok(id) => {
            println!("settled #{id}");
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// `memory status`: the unproven writes, per creature.
fn memory(args: &[String]) -> ExitCode {
    use mnemorph_server::world::memory::{Unproven, audit::SHARED, pathology};
    let mut world = None;
    let mut json = false;
    let mut verb = None;
    let mut it = args.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--json" => json = true,
            "--world" => match it.next() {
                Some(v) => world = Some(PathBuf::from(v)),
                None => return refused(&"--world needs a value"),
            },
            "status" if verb.is_none() => verb = Some(None),
            "wake" if verb.is_none() => match it.next() {
                Some(id) => verb = Some(Some(("wake", id.clone()))),
                None => return refused(&"wake needs the note's id"),
            },
            "confirm" if verb.is_none() => match it.next() {
                Some(id) => verb = Some(Some(("confirm", id.clone()))),
                None => return refused(&"confirm needs the claim's id"),
            },
            other => return refused(&format!("unknown argument `{other}`; see --help")),
        }
    }
    let Some(verb) = verb else {
        return refused(&"memory needs a verb: status, wake <note> or confirm <claim>");
    };
    let Ok(cwd) = std::env::current_dir() else {
        return refused(&"the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    match verb {
        Some(("confirm", id)) => return confirm(&world, &id),
        Some((_, id)) => return wake(&world, &id),
        None => {}
    }
    let report = match mnemorph_server::world::memory::status(&world) {
        Ok(r) => r,
        Err(e) => return refused(&format!("memory cannot be read: {e}")),
    };
    let frozen = report.iter().any(|(_, f)| !f.is_empty());
    // The traces' findings beside the unproven writes: a creature with traces alone is listed too.
    let mut rows: Vec<(String, Vec<Unproven>, Vec<String>)> = report
        .into_iter()
        .map(|(name, found)| (name, found, Vec::new()))
        .collect();
    for name in pathology::traced(&world) {
        if !rows.iter().any(|(n, _, _)| *n == name) {
            rows.push((name, Vec::new(), Vec::new()));
        }
    }
    // By name, the shared row last.
    rows.sort_by(|a, b| (a.0 == SHARED, &a.0).cmp(&(b.0 == SHARED, &b.0)));
    for (name, _, seen) in &mut rows {
        if name != SHARED {
            *seen = pathology::findings(&pathology::traces(&world, name).unwrap_or_default());
            // What the gate retired, for the human to review — and to wake when it was wrong.
            for (id, text) in
                mnemorph_server::world::memory::retired(&world, name).unwrap_or_default()
            {
                seen.push(format!(
                    "retired, kept dormant: {id} — {text} (undo: mnemorphd memory wake {id})"
                ));
            }
            // What rests on something that moved, until a route grounds it again.
            seen.extend(needing_review(&world, name));
            // What waits for a route, newest first: the human may vouch for any of it. JSON
            // gives it a field of its own: a claim waiting is no pathology.
            if json {
                continue;
            }
            let open = proposed_of(&world, name);
            for (id, text) in open.iter().take(PROPOSED_SHOWN) {
                seen.push(format!(
                    "proposed: {id} — {text} (vouch: mnemorphd memory confirm {id})"
                ));
            }
            if open.len() > PROPOSED_SHOWN {
                seen.push(format!("proposed: {} more", open.len() - PROPOSED_SHOWN));
            }
        }
    }
    if json {
        print_memory_json(&world, &rows);
    } else if rows.is_empty() {
        println!("no creature has memory here yet");
        session_verify_note(&world);
    } else {
        println!("{}", standing(&world));
        for (name, found, seen) in &rows {
            print_memory(name, found, seen);
        }
        session_verify_note(&world);
    }
    if frozen {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// `memory status --json`: each creature's unproven writes, pathologies and proposed claims.
fn print_memory_json(
    world: &Path,
    rows: &[(
        String,
        Vec<mnemorph_server::world::memory::Unproven>,
        Vec<String>,
    )],
) {
    use mnemorph_server::world::memory::audit::SHARED;
    let rows: Vec<serde_json::Value> = rows
        .iter()
        .map(|(name, found, seen)| {
            serde_json::json!({
                "creature": name,
                "frozen": !found.is_empty() && name != SHARED,
                "unproven": found.iter().map(|u| serde_json::json!({
                    "file": u.file, "event": u.event, "note": u.note, "why": u.why,
                })).collect::<Vec<_>>(),
                "pathologies": seen,
                "proposed": proposed_of(world, name).iter().map(|(id, text)| {
                    serde_json::json!({"id": id, "text": text})
                }).collect::<Vec<_>>(),
            })
        })
        .collect();
    println!("{}", serde_json::Value::Array(rows));
}

/// The line `memory status` adds when the session's card has no verify line: no claim of a solo
/// session can be grounded until it has one.
fn session_verify_note(world: &Path) {
    let roster = mnemorph_server::world::Roster::load(world);
    if let Some(why) = mnemorph_server::world::verify::missing_session_verify(world, &roster) {
        println!("note: {why}");
    }
}

/// A text a model or a document wrote, as a printed field shows it: its line breaks folded to
/// spaces (`wire::one_line`), then every character a reader could not see escaped
/// (`wire::shown`), so no field can open a line, redraw the terminal or read as another.
fn field(text: &str) -> String {
    mnemorph_server::wire::shown(&mnemorph_server::wire::one_line(text))
}

/// A creature's claims that need review — something they rest on moved — each a `memory status`
/// line with how to ground it again.
fn needing_review(world: &Path, name: &str) -> Vec<String> {
    mnemorph_server::world::memory::under_review(world, name)
        .unwrap_or_default()
        .into_iter()
        .map(|(id, text, cause)| {
            // One line each: a note's text and its cause may hold newlines, and whatever a reader
            // could not see is escaped.
            let one_line = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
            let text = field(&one_line(&text).chars().take(100).collect::<String>());
            let cause = field(&one_line(&cause));
            format!(
                "needs review: {id} — {text} ({cause}; ground it again: mnemorphd memory confirm \
                 {id})"
            )
        })
        .collect()
}

/// How many proposed claims `memory status` shows per creature.
const PROPOSED_SHOWN: usize = 10;

/// A creature's proposed claims, newest first: id and text.
fn proposed_of(world: &Path, name: &str) -> Vec<(String, String)> {
    mnemorph_server::world::memory::proposals(world, name)
        .unwrap_or_default()
        .into_iter()
        .map(|n| (n.id, n.text.chars().take(100).collect()))
        .collect()
}

/// `memory confirm <claim>`: the human vouches for a proposed claim — the **confirmed** route —
/// and it is grounded and shared, citing their word. A body never confirms one.
fn confirm(world: &Path, id: &str) -> ExitCode {
    use mnemorph_server::world::memory::{pending, seen, vouch};
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: confirming a claim is the human's act, never a body's"
        ));
    }
    // What is vouched for is shown, and its digest carried: a claim rewritten meanwhile is refused.
    let text = match pending(world, id) {
        Ok(Some(claim)) => claim.text,
        Ok(None) => String::new(),
        Err(e) => return refused(&e),
    };
    if !text.is_empty() {
        println!("confirming {}: {}", field(id), field(&text));
    }
    match vouch(world, id, "the human's word", &seen(id, &text)) {
        Ok(owner) => {
            println!("confirmed {id}: {owner}'s claim, grounded and shared");
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// `memory wake <note>`: the human restores a note the gate retired. A body never wakes one.
fn wake(world: &Path, id: &str) -> ExitCode {
    use mnemorph_server::world::memory::{retired_note, seen, wake_by};
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: waking a note is the human's act, never a body's"
        ));
    }
    // What is woken is shown, and its digest carried: a note rewritten meanwhile is refused.
    let text = match retired_note(world, id) {
        Ok(t) => t.unwrap_or_default(),
        Err(e) => return refused(&e),
    };
    if !text.is_empty() {
        println!("waking {}: {}", field(id), field(&text));
    }
    match wake_by(world, id, "woken by the human", &seen(id, &text)) {
        Ok(files) => {
            println!("woke {id}: grounded again in {}", files.join(", "));
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// How much of the world's memory stands: grounded in shared memory, and kept personal — the
/// first number to read after a run, since only shared notes reach other bodies.
fn standing(world: &Path) -> String {
    let s = mnemorph_server::world::memory::standing(world);
    format!(
        "memory: {} note(s) shared and grounded; personal: {} proposed, {} grounded",
        s.shared, s.proposed, s.grounded
    )
}

/// One creature's `memory status` lines: clear, or what does not resolve and what its recalls show.
fn print_memory(name: &str, found: &[mnemorph_server::world::memory::Unproven], seen: &[String]) {
    if found.is_empty() && seen.is_empty() {
        println!("{name}: clear");
        return;
    }
    if found.is_empty() {
        println!("{name}: writes free");
    } else {
        let state = if name == mnemorph_server::world::memory::audit::SHARED {
            "recall ignores"
        } else {
            "frozen —"
        };
        println!(
            "{name}: {state} {} note event(s) that do not resolve; remove or re-prove each",
            found.len()
        );
        for u in found {
            println!(
                "  {}  event {}  note {}: {}",
                u.file, u.event, u.note, u.why
            );
        }
    }
    for line in seen {
        println!("  {line}");
    }
}

/// A queue refusal: said on stderr, exit 2.
fn refused(e: &dyn std::fmt::Display) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "mnemorphd: refused: {e}");
    ExitCode::from(2)
}

/// `incident …`: the human lists the Kijin's open incidents, marks one, settles a reconcile that
/// is none, or records an incident's lesson. A body never does.
fn incident(args: &[String]) -> ExitCode {
    use mnemorph_server::incident;
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: an incident is the human's to mark, settle or close"
        ));
    }
    let mut world = None;
    let mut why = None;
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--world" => match it.next() {
                Some(v) => world = Some(PathBuf::from(v)),
                None => return refused(&"--world needs a value"),
            },
            "--why" => match it.next() {
                Some(v) => why = Some(v.clone()),
                None => return refused(&"--why needs a value"),
            },
            _ => words.push(w.clone()),
        }
    }
    let Ok(cwd) = std::env::current_dir() else {
        return refused(&"the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    // The witness is found again by the world's path, as the session names it.
    let world = match world.canonicalize() {
        Ok(w) if w.is_dir() => w,
        Ok(w) => return refused(&format!("{} is not a directory", w.display())),
        Err(e) => return refused(&format!("{}: {e}", world.display())),
    };
    let done = |r: Result<String, String>| match r {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    };
    match words.as_slice() {
        [verb] if verb == "list" => {
            let open = incident::list(&world);
            if open.is_empty() {
                println!("no incident is open");
            }
            for line in &open {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        [verb, post, act, rest @ ..] => {
            let post = match incident::post(post) {
                Ok(p) => p,
                Err(e) => return refused(&e),
            };
            match (verb.as_str(), rest) {
                ("open", []) => done(incident::open(&world, post, act, why.as_deref())),
                ("settle", []) => done(incident::settle(&world, post, act, why.as_deref())),
                ("lesson", rest) => match incident::lesson(&world, post, act, rest) {
                    Ok((lesson, change)) => {
                        // The human reads what the lesson changed before it is recorded.
                        println!("{change}");
                        print!("record {lesson} as the lesson of act {act}? [y/N] ");
                        let _ = std::io::stdout().flush();
                        let mut answer = String::new();
                        let _ = std::io::stdin().read_line(&mut answer);
                        if answer.trim().eq_ignore_ascii_case("y") {
                            done(incident::record(&world, post, act, lesson))
                        } else {
                            refused(&"the lesson was not recorded")
                        }
                    }
                    Err(e) => refused(&e),
                },
                _ => refused(&"see --help: incident list | open | settle | lesson"),
            }
        }
        _ => refused(&"incident needs a verb: list, open, settle or lesson; see --help"),
    }
}

/// `envelope …`: the human lists the Oni's envelopes, or revokes one. A body never does.
fn envelope(args: &[String]) -> ExitCode {
    use mnemorph_server::envelope;
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: an envelope is the human's to sign and revoke"
        ));
    }
    let mut world = None;
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--world" => match it.next() {
                Some(v) => world = Some(PathBuf::from(v)),
                None => return refused(&"--world needs a value"),
            },
            _ => words.push(w.clone()),
        }
    }
    let Ok(cwd) = std::env::current_dir() else {
        return refused(&"the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    let world = match world.canonicalize() {
        Ok(w) if w.is_dir() => w,
        Ok(w) => return refused(&format!("{} is not a directory", w.display())),
        Err(e) => return refused(&format!("{}: {e}", world.display())),
    };
    let listed = || match envelope::list(&world) {
        Ok(lines) => Ok(lines.join("\n")),
        Err(e) => Err(e),
    };
    match words.as_slice() {
        [verb] if verb == "list" => match listed() {
            Ok(text) => {
                println!("{text}");
                ExitCode::SUCCESS
            }
            Err(e) => refused(&e),
        },
        [verb, name] if verb == "revoke" => {
            // The human reads what they revoke before it is written.
            match listed() {
                Ok(text) => println!("{text}"),
                Err(e) => return refused(&e),
            }
            let safe = mnemorph_server::challenge::safe;
            match at_terminal(&format!("revoke envelope {}?", safe(name))) {
                Ok(true) => {}
                Ok(false) => return refused(&"the envelope was not revoked"),
                Err(e) => return refused(&e),
            }
            match envelope::revoke(&world, name) {
                Ok(line) => {
                    println!("{line}");
                    ExitCode::SUCCESS
                }
                Err(e) => refused(&e),
            }
        }
        _ => refused(&"envelope needs a verb: list, or revoke <name>; see --help"),
    }
}

/// `challenge …`: the human lists the Dark Elf challenges, shows one, or waives one with a reason.
/// A body never does.
fn challenge(args: &[String]) -> ExitCode {
    use mnemorph_server::world::challenge::{Challenges, Waiver};
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: a challenge is the human's to waive, never a body's"
        ));
    }
    let (mut world, mut why, mut all) = (None, None, false);
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--world" => match it.next() {
                Some(v) => world = Some(PathBuf::from(v)),
                None => return refused(&"--world needs a value"),
            },
            "--why" => match it.next() {
                Some(v) => why = Some(v.clone()),
                None => return refused(&"--why needs a value"),
            },
            "--all" => all = true,
            _ => words.push(w.clone()),
        }
    }
    let Ok(cwd) = std::env::current_dir() else {
        return refused(&"the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    let world = world.canonicalize().unwrap_or(world);
    let Some(records) = Challenges::of(&world) else {
        return refused(&"no state directory: the challenges cannot be read");
    };
    match words.as_slice() {
        [verb] if verb == "list" => {
            let shown: Vec<String> = records
                .records()
                .values()
                .filter(|r| all || r.status.open())
                .map(mnemorph_server::challenge::said)
                .collect();
            if shown.is_empty() {
                println!("no challenge holds a landing");
            }
            for line in shown {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        [verb, id] if verb == "show" => match records.record(id) {
            Some(r) => {
                println!("{}", full(&r));
                ExitCode::SUCCESS
            }
            None => refused(&format!("no challenge is open for landing `{id}`")),
        },
        [verb, id] if verb == "waive" => {
            let Some(why) = why else {
                return refused(&"a waiver says why: --why <reason>");
            };
            let waiver = match Waiver::human(&why) {
                Ok(w) => w,
                Err(e) => return refused(&e),
            };
            let Some(r) = records.record(id) else {
                return refused(&format!("no challenge is open for landing `{id}`"));
            };
            // The human reads what they waive before it is recorded.
            println!("{}", full(&r));
            match at_terminal(&format!("waive the challenge of {id}?")) {
                Ok(true) => {}
                Ok(false) => return refused(&"the challenge was not waived"),
                Err(e) => return refused(&e),
            }
            match records.waive(id, &waiver) {
                Ok(_) => {
                    println!("waived {id}: {}", waiver.reason());
                    ExitCode::SUCCESS
                }
                Err(e) => refused(&e),
            }
        }
        [verb] if verb == "score" => match mnemorph_server::world::challenge::scored(&world) {
            Ok((folded, chain)) => {
                let roster = mnemorph_server::world::Roster::load(&world);
                for line in score(&folded, &roster, &records, chain.as_ref()) {
                    println!("{line}");
                }
                ExitCode::SUCCESS
            }
            Err(e) => refused(&e),
        },
        [verb, id, fp, answer] if verb == "decide" => {
            decide(&records, id, fp, answer, why.as_deref())
        }
        _ => refused(&"challenge needs a verb: list, show, waive, score or decide; see --help"),
    }
}

/// `challenge decide`: the human's word on one finding of `id`, for the Dark Elf's score: shown
/// first, escaped, and asked for `y` at the terminal.
fn decide(
    records: &mnemorph_server::world::challenge::Challenges,
    id: &str,
    fp: &str,
    answer: &str,
    why: Option<&str>,
) -> ExitCode {
    use mnemorph_server::world::challenge::Decision;
    let upheld = match answer {
        "upheld" => true,
        "false-positive" => false,
        _ => return refused(&"a decision is `upheld` or `false-positive`"),
    };
    let Some(why) = why else {
        return refused(&"a decision says why: --why <reason>");
    };
    let decision = match Decision::human(upheld, why) {
        Ok(d) => d,
        Err(e) => return refused(&e),
    };
    let safe = mnemorph_server::challenge::safe;
    let Some(f) = records
        .record(id)
        .and_then(|r| r.findings.into_iter().find(|f| f.fingerprint() == fp))
    else {
        return refused(&format!(
            "no finding #{} on landing `{}`",
            safe(fp),
            safe(id)
        ));
    };
    // The human reads the finding before their word on it is recorded.
    println!(
        "#{} {} ({}): {} — {}",
        safe(fp),
        f.severity().word(),
        safe(&f.found.record.speciality),
        safe(&f.found.record.claim),
        f.verdict
            .as_ref()
            .map_or("no verdict yet", |v| v.ruling.word())
    );
    match at_terminal(&format!("record it as {answer}?")) {
        Ok(true) => {}
        Ok(false) => return refused(&"nothing was recorded"),
        Err(e) => return refused(&e),
    }
    match records.decide(id, fp, &decision) {
        Ok(()) => {
            println!("decided #{}: {answer} — {}", safe(fp), decision.reason());
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// Each speciality's value ledger, as `challenge score` says it, escaped: the counts; precision,
/// the upheld rate on the race's meter, the last ten and the `cannot-verify` rate, each with its
/// count and 95% interval; the builds its findings were settled under; and what is proposed to
/// the human, one line per bar that decided it, with the records' chain head and the rows of the
/// window it read — after the records' unreadable rows, and a chain that does not hold, when
/// there are any.
fn score(
    folded: &std::collections::BTreeMap<String, mnemorph_server::world::challenge::SpecialityFold>,
    roster: &mnemorph_server::world::Roster,
    records: &mnemorph_server::world::challenge::Challenges,
    chain: Option<&mnemorph_server::world::challenge::ChainReport>,
) -> Vec<String> {
    use mnemorph_server::world::challenge::Ascension;
    let safe = mnemorph_server::challenge::safe;
    let unreadable = records.unreadable();
    let mut lines = Vec::new();
    if !unreadable.is_empty() {
        let rows: Vec<String> = unreadable.iter().map(u64::to_string).collect();
        lines.push(format!(
            "the records hold unreadable rows ({}): what they held is not counted, and the \
             challenges they could be stay pending",
            rows.join(", ")
        ));
    }
    // A proposal names the records' chain head — walked over the very bytes the fold read — so it
    // can be recomputed from them.
    let head = match chain {
        Some(r) if r.holds() => r.head.as_ref().map(|h| h.digest.clone()),
        Some(r) => {
            lines.push(format!("the records' chain does not hold: {r}"));
            None
        }
        None => None,
    };
    let head = head.as_deref();
    if folded.is_empty() {
        lines.push("no Dark Elf finding is on record".to_owned());
        return lines;
    }
    for (speciality, f) in folded {
        // The form is the living body's that bears the speciality's name, when one is born.
        let form = roster
            .living()
            .find(|c| &c.name == speciality)
            .map(|c| c.race);
        lines.push(format!(
            "{} ({}): {} findings — {} upheld, {} refuted, {} cannot-verify, {} held for the \
             human, {} unverified",
            safe(speciality),
            form.map_or("no living body", |r| r.name()),
            f.findings,
            f.upheld,
            f.refuted,
            f.cannot_verify,
            f.held,
            f.unverified
        ));
        let m = f.meter();
        lines.push(format!(
            "  precision {} · {} {} ({})",
            f.precision_rate(),
            m.label.to_lowercase(),
            m.rate,
            format!("{:?}", m.band).to_lowercase(),
        ));
        lines.push(format!(
            "  last {} settled: precision {} · last {} filed: cannot-verify {}",
            f.recent.len(),
            f.recent_rate(),
            f.last_filed,
            f.cannot_verify_counts()
        ));
        let builds: Vec<String> = f
            .builds
            .iter()
            .map(|(b, n)| format!("{} {n}", safe(&b.to_string())))
            .collect();
        lines.push(format!("  settled under: {}", builds.join(" · ")));
        if let Some(p) = form.and_then(|r| f.proposed(r, head)) {
            let what = match p.ascension {
                Ascension::Rise => "rise to Arch Dark Elf",
                Ascension::Probation => {
                    "probation: precision under 70% over the last ten settled, or cannot-verify \
                     over 50% of the last ten filed"
                }
            };
            lines.push(format!(
                "  proposed to the human: {what} — nothing changes until the human applies it"
            ));
            for why in &p.why {
                lines.push(format!("    {}", safe(&why.to_string())));
            }
        }
    }
    lines
}

/// One challenge in full, as `show` and the waiver say it, escaped.
fn full(r: &mnemorph_server::world::challenge::Record) -> String {
    use mnemorph_server::world::challenge::Staff;
    let mut lines = vec![mnemorph_server::challenge::said(r)];
    // What a model chose — paths, names, reasons — is escaped before a terminal shows it.
    let safe = mnemorph_server::challenge::safe;
    lines.push(format!("  tree {}", safe(&r.opening.tree)));
    let wrote: Vec<String> = r.landing.written.iter().map(|p| safe(p)).collect();
    lines.push(format!("  wrote {}", wrote.join(", ")));
    if let Some(n) = r.opening.paths {
        lines.push(format!(
            "  (a sample of {n} paths, SHA-256 {})",
            safe(&r.opening.paths_sha256)
        ));
    }
    for (s, staff) in &r.staff {
        lines.push(match staff {
            Staff::Commissioned { body, by, wt, .. } => format!(
                "  {s}: reviewed by {}, sent by {}, {wt} wt",
                safe(body),
                safe(by)
            ),
            Staff::Queued { why } => format!("  {s}: queued — {}", safe(why)),
            Staff::Unstaffed => format!("  {s}: unstaffed — no body born"),
        });
    }
    // A proposed seal is shown with its frozen witness: it is the human's to confirm.
    for p in r.findings.iter().filter_map(|f| f.proposal.as_ref()) {
        lines.push(format!(
            "  seal proposed on `{}` (#{} {}), confirmed by {}: witness `{}` frozen at SHA-256 {} \
             — nothing is sealed until the human confirms",
            safe(&p.file),
            safe(&p.fingerprint),
            p.severity.word(),
            safe(&p.verifier),
            safe(&p.witness.reference),
            safe(&p.witness.sha256)
        ));
    }
    lines.join("\n")
}

/// The human's `y`, read at the controlling terminal: piped input — a script, a body's command —
/// is no human's answer, so with no terminal the question is refused, never answered.
fn at_terminal(question: &str) -> Result<bool, String> {
    use std::io::{BufRead as _, IsTerminal as _};
    let tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|e| format!("this is answered at a terminal, and there is none: {e}"))?;
    if !tty.is_terminal() {
        return Err("this is answered at a terminal, never piped".to_owned());
    }
    let mut out = tty.try_clone().map_err(|e| e.to_string())?;
    write!(out, "{question} [y/N] ")
        .and_then(|()| out.flush())
        .map_err(|e| e.to_string())?;
    let mut answer = String::new();
    std::io::BufReader::new(tty)
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    Ok(answer.trim().eq_ignore_ascii_case("y"))
}

/// `q …`: the shared work queue, for the human. Inside a body's command it refuses: a body moves
/// the queue only through the harness.
fn queue(args: &[String]) -> ExitCode {
    use mnemorph_server::world::queue::{HUMAN, Item, Move, Queue};
    let a = match q_args(args) {
        Ok(a) => a,
        Err(e) => return fail(&e),
    };
    let Ok(cwd) = std::env::current_dir() else {
        return fail("the current directory cannot be read");
    };
    let world = a
        .world
        .clone()
        .map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    let world = world.canonicalize().unwrap_or(world);
    let q = Queue::of(&world);
    let roster = mnemorph_server::world::Roster::load(&world);
    let json = a.json;
    let print = |items: &[Item]| print_items(items, json);
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: a body moves the queue only through the harness"
        ));
    }
    let nth = |n: usize| a.words.get(n).cloned();
    let moved = |id: Option<String>, mv: Move| match id {
        None => fail("which item? give its id"),
        Some(id) => match q.move_item(&roster, HUMAN, &id, &mv) {
            Ok(i) => {
                print(&[i]);
                ExitCode::SUCCESS
            }
            Err(e) => refused(&e),
        },
    };
    match a.words.first().map(String::as_str) {
        Some("list") | None => {
            let items: Vec<Item> = if a.all {
                q.items().into_values().collect()
            } else {
                q.open()
            };
            print(&items);
            ExitCode::SUCCESS
        }
        Some("show") => match nth(1).and_then(|id| q.items().remove(&id)) {
            Some(i) => {
                print(std::slice::from_ref(&i));
                if !json {
                    for n in &i.notes {
                        println!("  {n}");
                    }
                }
                ExitCode::SUCCESS
            }
            None => refused(&"no such item on the queue"),
        },
        Some("add") => {
            let ask = a.words[1..].join(" ");
            if ask.is_empty() {
                return fail("q add needs the order's words");
            }
            match q.add(&roster, a.to.as_deref(), HUMAN, &ask, a.rank) {
                Ok(i) => {
                    print(&[i]);
                    ExitCode::SUCCESS
                }
                Err(e) => refused(&e),
            }
        }
        Some("claim") => moved(
            nth(1),
            Move::Claim {
                as_body: a.as_body.clone(),
            },
        ),
        Some("touch") if a.words.len() < 3 => fail("q touch needs the item's id and a note"),
        Some("touch") => moved(
            nth(1),
            Move::Touch {
                note: a.words.get(2..).map(|w| w.join(" ")).unwrap_or_default(),
            },
        ),
        Some("requeue") => moved(nth(1), Move::Requeue),
        Some("close") => moved(nth(1), Move::Close { drop: a.drop }),
        Some(other) => fail(&format!("q has no `{other}`; see --help")),
    }
}

/// `meters [--world <dir>] [--json]`: every living creature's meters, by race.
fn meters(args: &[String]) -> ExitCode {
    let (mut world, mut json) = (None, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => json = true,
            "--world" => match it.next() {
                Some(dir) => world = Some(PathBuf::from(dir)),
                None => return refused(&"--world needs a value"),
            },
            other => return refused(&format!("unknown argument `{other}`; see --help")),
        }
    }
    let Ok(cwd) = std::env::current_dir() else {
        return fail("the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    let world = world.canonicalize().unwrap_or(world);
    let w = mnemorph_server::world::Roster::load(&world);
    let folds = mnemorph_server::world::challenge::scored(&world)
        .map(|(f, _)| f)
        .unwrap_or_default();
    // The fold kept in the report ledger; a fold that cannot be trusted is shown unavailable.
    let (reports, kept) = match mnemorph_server::world::reports::record(&world) {
        Ok((f, path)) => (Ok(f), Ok(path)),
        Err(why) => (mnemorph_server::world::reports::fold(&world), Err(why)),
    };
    let all = mnemorph_server::world::meters::of(&w, &folds, &reports);
    let safe = mnemorph_server::challenge::safe;
    // What the compiled law proposes from the same journals: shown with the command that applies
    // it, never applied here.
    let survey = mnemorph_server::world::ascension::survey(&world);
    if json {
        for m in &all {
            let readings: Vec<serde_json::Value> = m
                .readings
                .iter()
                .map(|r| {
                    // No report is no count, never a zero; an unavailable fold says why.
                    let status = match r.rate {
                        None => "unavailable",
                        Some(x) if x.n == 0 => "noReports",
                        Some(_) => "counted",
                    };
                    let rate = r.rate.filter(|x| x.n > 0);
                    let (lo, hi) = rate.and_then(|x| x.interval()).unzip();
                    serde_json::json!({
                        "meter": r.meter,
                        "status": status,
                        "why": r.rate.is_none().then(|| m.unavailable.clone()).flatten(),
                        "successes": rate.map(|x| x.successes),
                        "n": rate.map(|x| x.n),
                        "value": rate.and_then(|x| x.value()),
                        "lo": lo,
                        "hi": hi,
                    })
                })
                .collect();
            let proposal = survey.as_ref().ok().and_then(|s| s.of(&m.name)).map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "act": p.act.word(),
                    "to": p.to.name(),
                    "endedProbation": p.ended_probation,
                    "reports": p.n,
                    "asOf": p.as_of.to_string(),
                    "inputsSha256": p.inputs_sha256,
                    "why": p.why.iter().map(ToString::to_string).collect::<Vec<_>>(),
                    "command": p.command(&world),
                })
            });
            let probation = survey
                .as_ref()
                .ok()
                .and_then(|s| s.probation_of(&m.name))
                .map(|p| serde_json::json!({"reports": p.reports, "since": p.began.to_string()}));
            println!(
                "{}",
                serde_json::json!({
                    "creature": m.name,
                    "race": m.race.name(),
                    "meters": readings,
                    "proposal": proposal,
                    "probation": probation,
                })
            );
        }
        return ExitCode::SUCCESS;
    }
    if all.is_empty() {
        println!("no living creature in this world");
        return ExitCode::SUCCESS;
    }
    for race in mnemorph_server::world::meters::Race::ALL {
        let of: Vec<_> = all.iter().filter(|m| m.race == race).collect();
        if of.is_empty() {
            continue;
        }
        println!("{}", race.name());
        for m in of {
            println!("  {}", safe(&m.to_string()));
        }
    }
    match &survey {
        Ok(s) if s.proposals.is_empty() && s.probation.is_empty() => {}
        Ok(s) => {
            println!("\nproposed by the compiled law — nothing changes until the human applies it");
            for line in mnemorph_server::world::ascension::lines_of(s, &world) {
                println!("  {}", safe(&line));
            }
        }
        Err(why) => println!("\nrises and falls unavailable: {}", safe(why)),
    }
    match kept {
        Ok(path) => println!(
            "\nreports folded from the trusted journals, kept in {}",
            safe(&path.display().to_string())
        ),
        Err(why) => println!("\nthe report ledger is not kept: {}", safe(&why)),
    }
    println!(
        "shown with their count and 95% interval, never judged on; no rise or fall is applied"
    );
    ExitCode::SUCCESS
}

/// Dollars as `usage` says them: `$0.4200`, `$0.4200 (2 unpriced)`, or `unpriced (2 calls)` when
/// none was priced — never $0.
fn dollars(t: &mnemorph_server::world::ledger::Totals) -> String {
    match mnemorph_server::world::spend::usd(t) {
        None => format!("unpriced ({} calls)", t.unpriced),
        Some(usd) if t.unpriced > 0 => format!("${usd:.4} ({} unpriced)", t.unpriced),
        Some(usd) => format!("${usd:.4}"),
    }
}

/// What the providers themselves reported the calls cost, beside Mnemorph's estimate:
/// ` · provider-reported $0.0000 (3 calls)`, or nothing when no call carried a report.
fn reported(t: &mnemorph_server::world::ledger::Totals) -> String {
    if t.reported == 0 {
        return String::new();
    }
    format!(
        " · provider-reported ${:.4} ({} call{})",
        t.reported_usd,
        t.reported,
        if t.reported == 1 { "" } else { "s" }
    )
}

/// A fold's totals as JSON: what the board's metrics carry for the same group.
fn totals_json(t: &mnemorph_server::world::ledger::Totals) -> serde_json::Value {
    use mnemorph_server::world::spend;
    serde_json::json!({
        "calls": t.calls,
        "wt": t.wt,
        "input": t.usage.input,
        "output": t.usage.output,
        "cacheRead": t.usage.cache_read,
        "cacheWrite": t.usage.cache_write,
        "usd": spend::usd(t),
        "unpriced": t.unpriced,
        "reportedUsd": (t.reported > 0).then_some(t.reported_usd),
        "reported": t.reported,
        "cacheHit": spend::cache_hit(t),
    })
}

/// `usage`'s text, escaped: the totals line, then by race, model, day and the latest sessions.
fn usage_lines(
    r: &mnemorph_server::world::spend::Reading,
    ledger: &mnemorph_server::world::spend::Ledger,
) -> Vec<String> {
    use mnemorph_server::world::spend;
    let safe = mnemorph_server::challenge::safe;
    let window = r.window;
    let mut lines = Vec::new();
    let hit = |t: &mnemorph_server::world::ledger::Totals| {
        spend::cache_hit(t).map_or_else(|| "no input".to_owned(), |h| format!("{:.0}%", h * 100.0))
    };
    let t = &r.total;
    lines.push(format!(
        "usage {}: {}{} · {} wt · {} calls · cache hit {}",
        window.word(),
        dollars(t),
        reported(t),
        t.wt,
        t.calls,
        hit(t)
    ));
    for (title, m) in [("race", &r.races), ("model", &r.models), ("day", &r.days)] {
        if m.is_empty() {
            continue;
        }
        lines.push(format!("by {title}"));
        for (k, t) in m {
            lines.push(format!(
                "  {}: {}{} · {} wt · {} calls",
                safe(k),
                dollars(t),
                reported(t),
                t.wt,
                t.calls
            ));
        }
    }
    if !r.sessions.is_empty() {
        lines.push("by session, the latest first".to_owned());
        for s in r.sessions.iter().take(20) {
            lines.push(format!(
                "  {} ({}): {} · {} wt · {} calls",
                safe(&s.id),
                s.last,
                dollars(&s.totals),
                s.totals.wt,
                s.totals.calls
            ));
        }
    }
    lines.push(match &ledger.why {
        None => format!(
            "read from the usage witness under the state directory, its chain holding: {}",
            safe(&ledger.path.display().to_string())
        ),
        Some(why) => format!("not trusted: {}", safe(why)),
    });
    if ledger.skipped > 0 {
        lines.push(format!(
            "{} line(s) of the ledger do not read and are not counted",
            ledger.skipped
        ));
    }
    if ledger.cut {
        lines.push(format!(
            "read from the ledger's last {} MiB: older calls are not counted",
            spend::READ_CAP >> 20
        ));
    }
    lines
}

/// `usage`'s MCP lines, escaped: each server's schema loaded against used and its answers' cost,
/// from the MCP journal under the state directory — or why it is not believed. Nothing when no
/// MCP was ever journaled.
fn mcp_lines(mcp: &Result<mnemorph_server::mcp_journal::SpendReport, String>) -> Vec<String> {
    let safe = mnemorph_server::challenge::safe;
    match mcp {
        Ok((servers, _)) if servers.is_empty() => Vec::new(),
        Ok((servers, (calls, wt))) => {
            let mut lines = vec!["mcp schema, by server".to_owned()];
            for (name, s) in servers {
                lines.push(format!(
                    "  {}: {} wt of schema loaded, {} wt of it used; {} wt of answers",
                    safe(name),
                    s.loaded,
                    s.used,
                    s.answers
                ));
            }
            if *calls > 0 {
                lines.push(format!("  rerank: {calls} calls, {wt} wt"));
            }
            lines
        }
        Err(why) => vec![format!("mcp: the journal is not believed: {}", safe(why))],
    }
}

/// `usage [--window 7d|28d|all] [--world <dir>] [--json]`: the usage ledger's fold.
fn usage(args: &[String]) -> ExitCode {
    use mnemorph_server::world::spend::{self, Window};
    let (mut world, mut json, mut window) = (None, false, Window::Week);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => json = true,
            "--world" => match it.next() {
                Some(dir) => world = Some(PathBuf::from(dir)),
                None => return refused(&"--world needs a value"),
            },
            "--window" => match it.next().and_then(|w| Window::parse(w)) {
                Some(w) => window = w,
                None => return refused(&"--window is 7d, 28d or all"),
            },
            other => return refused(&format!("unknown argument `{other}`; see --help")),
        }
    }
    let Ok(cwd) = std::env::current_dir() else {
        return fail("the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    let world = world.canonicalize().unwrap_or(world);
    let ledger = match spend::read(&world) {
        Ok(l) => l,
        Err(e) => return fail(&format!("the usage ledger cannot be read: {e}")),
    };
    let now = mnemorph_server::world::ledger::Timestamp::now();
    let r = spend::fold(&ledger.rows, window, now);
    let groups =
        |m: &std::collections::BTreeMap<String, mnemorph_server::world::ledger::Totals>| {
            m.iter()
                .map(|(k, t)| (k.clone(), totals_json(t)))
                .collect::<serde_json::Map<String, serde_json::Value>>()
        };
    let mcp = mnemorph_server::mcp_journal::spend(&world);
    if json {
        let mcp_json = match &mcp {
            Ok((servers, (calls, wt))) => serde_json::json!({
                "trusted": true,
                "servers": servers
                    .iter()
                    .map(|(name, s)| (name.clone(), serde_json::json!({
                        "schemaLoadedWt": s.loaded,
                        "schemaUsedWt": s.used,
                        "answersWt": s.answers,
                    })))
                    .collect::<serde_json::Map<String, serde_json::Value>>(),
                "reranks": {"calls": calls, "wt": wt},
            }),
            Err(why) => serde_json::json!({"trusted": false, "why": why}),
        };
        let sessions: serde_json::Map<String, serde_json::Value> = r
            .sessions
            .iter()
            .map(|s| (s.id.clone(), totals_json(&s.totals)))
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "window": window.word(),
                "total": totals_json(&r.total),
                "races": groups(&r.races),
                "models": groups(&r.models),
                "bodies": groups(&r.bodies),
                "days": groups(&r.days),
                "sessions": sessions,
                "burnPerMinute": r.burn_per_minute(),
                "skipped": ledger.skipped,
                "cut": ledger.cut,
                "source": ledger.path.display().to_string(),
                "trusted": ledger.trusted,
                "why": ledger.why,
                "mcp": mcp_json,
            })
        );
        return ExitCode::SUCCESS;
    }
    for line in usage_lines(&r, &ledger) {
        println!("{line}");
    }
    for line in mcp_lines(&mcp) {
        println!("{line}");
    }
    ExitCode::SUCCESS
}

/// `evolution swap|judge`: the human swaps an accepted plan in, or judges a swap.
fn evolution(args: &[String]) -> ExitCode {
    use mnemorph_server::world::evolution::{Arm, Plan};
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: an evolution is the human's to swap in and judge, never a body's"
        ));
    }
    let (mut world, mut creature, mut arm) = (None, None, Arm::Mutate);
    let (mut bodies, mut proven) = (Vec::new(), Vec::new());
    let mut set = mnemorph_server::world::evolution::FileSet::default();
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(w) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        let got = match w.as_str() {
            "--world" => value("--world").map(|v| world = Some(PathBuf::from(v))),
            "--creature" => value("--creature").map(|v| creature = Some(v)),
            "--body" => value("--body").map(|v| bodies.push(v)),
            "--file" => value("--file").map(|v| set.replaces.push(v)),
            "--create" => value("--create").map(|v| set.creates.push(v)),
            "--remove" => value("--remove").map(|v| set.removes.push(v)),
            "--proven-on" => value("--proven-on")
                .map(|v| proven.push(mnemorph_server::world::evolution::task_of(&v))),
            "--arm" => value("--arm").and_then(|v| {
                arm = match v.as_str() {
                    "mutate" => Arm::Mutate,
                    "split" => Arm::Split,
                    "rebirth" => Arm::Rebirth,
                    other => return Err(format!("`{other}` is no arm: mutate, split or rebirth")),
                };
                Ok(())
            }),
            other if other.starts_with("--") => {
                Err(format!("unknown argument `{other}`; see --help"))
            }
            _ => {
                words.push(w.clone());
                Ok(())
            }
        };
        if let Err(e) = got {
            return refused(&e);
        }
    }
    let Ok(cwd) = std::env::current_dir() else {
        return fail("the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    let world = world.canonicalize().unwrap_or(world);
    match words.as_slice() {
        [verb, id] if verb == "swap" => {
            let Some(creature) = creature else {
                return refused(&"a swap names its creature: --creature <name>");
            };
            let files = match mnemorph_server::world::evolution::staged(&world, id, &set) {
                Ok(f) => f,
                Err(e) => return refused(&e),
            };
            if bodies.is_empty() {
                bodies.push(creature.clone());
            }
            let plan = Plan {
                id: id.clone(),
                creature,
                bodies,
                arm,
                files,
                proven_on: proven,
            };
            swap_in(&world, &plan)
        }
        [verb, id] if verb == "judge" => judge_swap(&world, id),
        _ => refused(&"evolution needs a verb: swap <id> or judge <id>; see --help"),
    }
}

/// `mind list | adopt <name> [--owner <creature>] | retire <name> | unretire <name>`: the
/// world's Minds against their witness, and the human's acts on one. Listing is anyone's; an act
/// is the human's, asked at the terminal, and refused inside a body's command.
fn mind(args: &[String]) -> ExitCode {
    use mnemorph_server::world::minds::{self, witness};
    let (mut world, mut owner, mut words) = (None, None, Vec::new());
    let mut it = args.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--world" | "--owner" => {
                let Some(v) = it.next() else {
                    return refused(&format!("{w} needs a value"));
                };
                if w == "--world" {
                    world = Some(PathBuf::from(v));
                } else {
                    owner = Some(v.clone());
                }
            }
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
    let (act, name) = match words.as_slice() {
        ["list"] | [] => {
            if owner.is_some() {
                return refused(&"--owner is named on adoption only");
            }
            let lines = minds::survey(&world).lines();
            if lines.is_empty() {
                println!("no Mind under .mnemorph/minds/");
            }
            for line in lines {
                println!("{line}");
            }
            return ExitCode::SUCCESS;
        }
        [verb, name] => match witness::Act::from_word(verb) {
            Some(act) => (act, *name),
            None => return refused(&format!("mind has no `{verb}`; see --help")),
        },
        _ => return refused(&"mind needs list, or adopt|retire|unretire <name>; see --help"),
    };
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: a Mind is adopted, retired and unretired by the human alone"
        ));
    }
    let Some(at) = witness::path_of(&world) else {
        return refused(&"no home: the Minds' witness is kept under the human's state directory");
    };
    // The doc is read once: the text shown is exactly the bytes whose digest is recorded.
    let minds::Proposal { row, text } =
        match minds::propose_at(&world, &at, act, name, owner.as_deref()) {
            Ok(p) => p,
            Err(e) => return refused(&e),
        };
    let safe = mnemorph_server::challenge::safe;
    println!("{} the Mind `{}`:", act.word(), safe(&row.mind));
    println!("  doc     {}", safe(&row.doc));
    println!("  sha256  {}", row.sha256);
    if let Some(o) = &row.owner {
        println!("  owner   {}", safe(o));
    }
    for line in shown_doc(&text) {
        println!("  │ {line}");
    }
    match at_terminal(&format!("{} `{}`?", act.word(), safe(&row.mind))) {
        Ok(true) => {}
        Ok(false) => return refused(&"nothing was recorded"),
        Err(e) => return refused(&e),
    }
    let done = match act {
        witness::Act::Adopt => "adopted: it loads while its doc holds these bytes",
        witness::Act::Retire => "retired: it no longer loads",
        witness::Act::Unretire => "unretired: it loads while its doc holds the adopted bytes",
    };
    match witness::record_at(&at, row) {
        Ok(()) => {
            println!("{} {done}", safe(name));
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// `verify list|pin`: the verify pins, for the human.
fn verify(args: &[String]) -> ExitCode {
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
        ["list"] | [] if !yes => verify_list(&world),
        ["list"] | [] => refused(&"--yes is given with pin only"),
        ["pin", name] => verify_pin(&world, name, yes),
        _ => refused(&"verify needs list, or pin <creature>; see --help"),
    }
}

/// `verify list`: each living creature's card lines, and whether its gate runs them.
fn verify_list(world: &Path) -> ExitCode {
    use mnemorph_server::world::{Roster, pins};
    let safe = mnemorph_server::challenge::safe;
    let held = match pins::pinned(world) {
        Ok(Some(p)) => p,
        Ok(None) => {
            return refused(&"no home: the verify pins are kept under the human's state directory");
        }
        Err(e) => return refused(&e),
    };
    let mut any = false;
    let all = Ok(Some(held.clone()));
    for c in Roster::load(world)
        .creatures
        .values()
        .filter(|c| !c.retired)
    {
        let pinned = held.get(&c.name);
        if c.verify.is_empty() && pinned.is_none() {
            continue;
        }
        any = true;
        let state = match pins::trusted_in(&all, &c.name, &c.doc(), &c.verify) {
            Ok(_) => "pinned",
            Err(_) if pinned.is_none() => "not pinned: its gate runs nothing",
            Err(_) => "differs from the pin: its gate runs nothing",
        };
        println!("{} — {state}", safe(&c.name));
        for line in &c.verify {
            println!("  card    `{}`", safe(line));
        }
        for p in pinned.into_iter().flatten() {
            println!("  pinned  `{}` — by {}", safe(&p.line), safe(&p.by));
        }
    }
    if !any {
        println!("no creature's card holds a verify line");
    }
    ExitCode::SUCCESS
}

/// `verify pin <creature>`: the card's lines shown, asked at the terminal unless `yes`, pinned.
fn verify_pin(world: &Path, name: &str, yes: bool) -> ExitCode {
    use mnemorph_server::world::{Roster, pins};
    let safe = mnemorph_server::challenge::safe;
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: a verify line is pinned by the human alone"
        ));
    }
    let roster = Roster::load(world);
    let Some(c) = roster.creatures.get(name).filter(|c| !c.retired) else {
        return refused(&format!("no living creature is named `{}`", safe(name)));
    };
    if let Err(e) = pins::pinned(world) {
        return refused(&e);
    }
    // The card is read once: the lines shown are exactly the lines pinned.
    let lines = c.verify.clone();
    println!(
        "pin the verify lines of `{}` ({}):",
        safe(&c.name),
        safe(&c.doc())
    );
    if lines.is_empty() {
        println!("  (none: its gate runs no check, and a line added later runs only once pinned)");
    }
    for line in &lines {
        println!("  `{}`", safe(line));
    }
    if !yes {
        match at_terminal(&format!(
            "the end gate of `{}` runs these on every landing — pin them?",
            safe(&c.name)
        )) {
            Ok(true) => {}
            Ok(false) => return refused(&"nothing was pinned"),
            Err(e) => return refused(&e),
        }
    }
    match pins::record_human(
        world,
        pins::row(pins::Act::Pin, &c.name, &lines, pins::HUMAN),
    ) {
        Ok(()) => {
            println!(
                "{} pinned: its gate runs these lines while its card says the same",
                safe(name)
            );
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// How many of a Mind's lines, and bytes of each, the adopt prompt shows.
const SHOWN_LINES: usize = 60;
const SHOWN_LINE_BYTES: usize = 200;

/// A Mind's text as the prompt shows it: each line escaped, the first [`SHOWN_LINES`] cut to
/// [`SHOWN_LINE_BYTES`], then how many more lines there are.
fn shown_doc(text: &str) -> Vec<String> {
    let safe = mnemorph_server::challenge::safe;
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = lines
        .iter()
        .take(SHOWN_LINES)
        .map(|l| {
            let mut cut = l.len().min(SHOWN_LINE_BYTES);
            while !l.is_char_boundary(cut) {
                cut -= 1;
            }
            let more = if cut < l.len() { " …" } else { "" };
            format!("{}{more}", safe(&l[..cut]))
        })
        .collect();
    if lines.len() > SHOWN_LINES {
        out.push(format!("… {} more lines", lines.len() - SHOWN_LINES));
    }
    out
}

/// Whether `path` is a creature's card: `.mnemorph/<race>/<seat>/README.md`.
fn is_card(path: &str) -> bool {
    let parts: Vec<&str> = path.trim_start_matches("./").split('/').collect();
    matches!(parts.as_slice(), [".mnemorph", race, _, "README.md"]
        if mnemorph_server::world::races::Race::from_name(race).is_some())
}

/// `evolution swap`: the plan shown, asked at the terminal, then swapped in and its row written.
fn swap_in(world: &Path, plan: &mnemorph_server::world::evolution::Plan) -> ExitCode {
    let safe = mnemorph_server::challenge::safe;
    println!(
        "swap {} for {} ({:?}), judged on {}:",
        safe(&plan.id),
        safe(&plan.creature),
        plan.arm,
        safe(&plan.bodies.join(", "))
    );
    for f in &plan.files {
        let what = f.after.as_ref().map_or_else(
            || "removed".to_owned(),
            |b| {
                let verb = if f.creates { "created" } else { "replaced" };
                format!("{verb}, {} bytes", b.len())
            },
        );
        println!("  {} — {what}", safe(&f.path));
    }
    // The yes is to bytes: what each card it writes would stand as is shown first.
    let cards: Vec<(String, String)> = plan
        .files
        .iter()
        .filter(|f| is_card(&f.path))
        .filter_map(|f| {
            let text = String::from_utf8_lossy(f.after.as_deref()?).into_owned();
            Some((f.path.clone(), text))
        })
        .collect();
    if !cards.is_empty() {
        println!("the cards it writes, as the creatures' witness would hold them:");
        for line in mnemorph_server::world::creatures::swap_standing(world, &cards) {
            println!("  {}", safe(&line));
        }
    }
    match at_terminal(&format!("swap {} in?", plan.id)) {
        Ok(true) => {}
        Ok(false) => return refused(&"nothing was swapped"),
        Err(e) => return refused(&e),
    }
    match mnemorph_server::world::evolution::swap(world, plan) {
        Ok(s) => {
            println!("swapped {} under build {}", safe(&s.id), s.build);
            if s.files
                .iter()
                .any(|f| f.path.starts_with(".mnemorph/minds/"))
            {
                println!(
                    "  a Mind it changed loads again once re-adopted: `mnemorphd mind list`, then \
                     `mnemorphd mind adopt <name>`"
                );
            }
            for f in &s.files {
                let short =
                    |d: &Option<String>| d.as_deref().map_or("absent", |d| &d[..12]).to_owned();
                println!(
                    "  {} {} → {}",
                    safe(&f.path),
                    short(&f.before),
                    short(&f.after)
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => refused(&e),
    }
}

/// `evolution judge`: the controls that wait run, then the judgement and what it proposes.
fn judge_swap(world: &Path, id: &str) -> ExitCode {
    let safe = mnemorph_server::challenge::safe;
    match mnemorph_server::evolution::judge(world, id, &|v| std::env::var(v).ok(), &|line| {
        println!("{}", mnemorph_server::challenge::safe(line));
    }) {
        Ok(j) => {
            for line in &j.controls {
                println!("{}", safe(line));
            }
            let (proposal, code) = match &j.judgement {
                mnemorph_server::world::evolution::Judgement::Keep(_) => {
                    (Some("keep"), ExitCode::SUCCESS)
                }
                mnemorph_server::world::evolution::Judgement::Rollback(_) => {
                    (Some("rollback"), ExitCode::SUCCESS)
                }
                mnemorph_server::world::evolution::Judgement::Pending { .. } => {
                    (None, ExitCode::SUCCESS)
                }
                mnemorph_server::world::evolution::Judgement::Refused(_) => {
                    (None, ExitCode::FAILURE)
                }
            };
            println!("{}", safe(&j.judgement.to_string()));
            if let Some(w) = proposal {
                println!("proposed to the human: {w} — nothing changes until the human applies it");
            }
            code
        }
        Err(e) => refused(&e),
    }
}

/// `path` made absolute against `cwd` and read segment by segment, without touching the disk:
/// a path that does not exist yet still has an owner.
fn absolute(cwd: &Path, path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in cwd.join(path).components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn fail(why: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "mnemorphd: {why}");
    ExitCode::FAILURE
}
