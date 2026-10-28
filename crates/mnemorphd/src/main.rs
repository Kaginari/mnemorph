//! `mnemorphd` — the Mnemorph core. It speaks the client protocol on stdio; the SDK and the board
//! are its clients, and decide nothing. It also holds the world's admin commands.

use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const HELP: &str = "mnemorphd — the Mnemorph core

Usage: mnemorphd [--world <dir>] [--resume <run|last>]
       mnemorphd runs [--world <dir>] [--json]
       mnemorphd onto owner <path>... [--world <dir>]
       mnemorphd onto check [--world <dir>]
       mnemorphd onto settle <fact-id> [--world <dir>]
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

Speaks the client protocol (JSON-RPC 2.0, one message per line) on stdin and stdout, for the
SDK and the board; the `mnemorph` command hosts its session in its own process. The admin
commands below are the world's, for the human.

`onto owner` names each path's owner, the rule that gave it and the claim that matched, read
from the creature docs; it needs no key and opens no session. A path is relative to the
current directory, or absolute. `onto check` prints `@S PASS|FAIL <n> findings`, then each
finding's severity, code, place and message; it fails on a critical or high finding, or a doc
that does not read.

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

`housekeep --verify` walks each chained journal — the usage ledger and the Dark Elf's value
ledger — once, held to the newest head anchored for it, and says for each: OK with its head, a
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

fn main() -> ExitCode {
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
    if all.first().map(String::as_str) == Some("onto") {
        return onto(&all[1..]);
    }
    if all.first().map(String::as_str) == Some("q") {
        return queue(&all[1..]);
    }
    if all.first().map(String::as_str) == Some("housekeep") {
        return housekeep(&all[1..]);
    }
    if all.first().map(String::as_str) == Some("relief") {
        return relief(&all[1..]);
    }
    if all.first().map(String::as_str) == Some("memory") {
        return memory(&all[1..]);
    }
    if all.first().map(String::as_str) == Some("runs") {
        return runs(&all[1..]);
    }
    if all.first().map(String::as_str) == Some("incident") {
        return incident(&all[1..]);
    }
    if all.first().map(String::as_str) == Some("envelope") {
        return envelope(&all[1..]);
    }
    if all.first().map(String::as_str) == Some("challenge") {
        return challenge(&all[1..]);
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
        Some(c @ ("owner" | "check" | "settle")) => c,
        Some(other) => return fail(&format!("onto has no `{other}`; see --help")),
        None => return fail("onto needs a command: owner, check or settle; see --help"),
    };
    while let Some(w) = words.next() {
        if w == "--world" {
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
    let Ok(cwd) = std::env::current_dir() else {
        return fail("the current directory cannot be read");
    };
    // Both sides canonical where they exist, so a symlinked path (macOS's `/var`) still matches.
    let canonical = |p: PathBuf| p.canonicalize().unwrap_or(p);
    let world = canonical(world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w)));
    let cwd = canonical(cwd);
    let roster = Roster::load(&world);
    for problem in &roster.problems {
        let _ = writeln!(std::io::stderr(), "mnemorphd: {problem}");
    }
    if command == "check" {
        return check(&world, &roster);
    }
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
        let _ = writeln!(out, "{path}\t{owner}\t{why}");
    }
    ExitCode::SUCCESS
}

/// `onto check`: every finding about the world's shape, most severe first. It fails on a
/// critical or high finding, or a doc that does not read.
fn check(world: &Path, roster: &mnemorph_server::world::Roster) -> ExitCode {
    let tracked = std::process::Command::new("git")
        .arg("-C")
        .arg(world)
        // `-z`: names as they are, never quoted (`src/café.rs`, not `"src/caf\303\251.rs"`).
        .args(["ls-files", "-z"])
        .output()
        .ok()
        .filter(|o| o.status.success());
    let tracked: Vec<String> = if let Some(o) = tracked {
        String::from_utf8_lossy(&o.stdout)
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect()
    } else {
        let _ = writeln!(
            std::io::stderr(),
            "mnemorphd: the world is no git repository; unowned soil is not checked"
        );
        Vec::new()
    };
    let mut findings = mnemorph_server::world::check(roster, &tracked);
    let facts = mnemorph_server::world::Facts::load(world);
    findings.extend(mnemorph_server::world::fact_findings(&facts, world));
    findings.extend(mnemorph_server::world::prose_claims(roster, world));
    findings.extend(mnemorph_server::world::mind_findings(roster, world));
    findings.extend(mnemorph_server::world::provenance(roster, world));
    if let Some(message) = mnemorph_server::world::verify::missing_session_verify(world, roster) {
        findings.push(mnemorph_server::world::Finding {
            code: "no-session-verify",
            severity: mnemorph_server::world::Severity::Low,
            at: ".mnemorph/rimuru/rimuru/README.md".into(),
            paths: Vec::new(),
            creatures: vec!["rimuru".into()],
            message,
        });
    }
    findings.sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.at.cmp(&b.at)));
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
            mnemorph_server::wire::one_line(&f.at),
            mnemorph_server::wire::one_line(&f.message)
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

/// `onto settle <id>`: the human settles a contested fact — besides an Orc's verdict, the one act
/// that does; recency never settles. A body never runs it.
fn settle(world: Option<&Path>, ids: &[String]) -> ExitCode {
    use mnemorph_server::world::Facts;
    use mnemorph_server::world::facts::Event;
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: settling a fact is the human's or an Orc's"
        ));
    }
    let [id] = ids else {
        return refused(&"onto settle takes one fact id: `#<id>` or `<id>`");
    };
    let id = id.trim_start_matches('#');
    let Ok(cwd) = std::env::current_dir() else {
        return refused(&"the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, w));
    if !Facts::load(&world).facts.contains_key(id) {
        return refused(&format!("no fact `#{id}` in this world's ontology"));
    }
    let event = Event::Settle {
        id: id.to_owned(),
        by: "human".into(),
        proof: "the human's word".into(),
    };
    match Facts::record(&world, &event) {
        Ok(()) => {
            println!("settled #{id}");
            ExitCode::SUCCESS
        }
        Err(e) => refused(&format!("the settle could not be filed: {e}")),
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

/// How many proposed claims `memory status` shows per creature.
const PROPOSED_SHOWN: usize = 10;

/// A creature's proposed claims, newest first: id and text.
fn proposed_of(world: &Path, name: &str) -> Vec<(String, String)> {
    use mnemorph_server::world::memory::{Status, Tier, fold, read};
    let mut open: Vec<_> = read(world, &Tier::Personal(name.to_owned()))
        .map(|r| fold(&r.rows))
        .unwrap_or_default()
        .into_values()
        .filter(|n| n.status == Status::Proposed && n.owner.as_deref() == Some(name))
        .collect();
    open.sort_by_key(|n| std::cmp::Reverse(n.born));
    open.into_iter()
        .map(|n| (n.id, n.text.chars().take(100).collect()))
        .collect()
}

/// `memory confirm <claim>`: the human vouches for a proposed claim — the **confirmed** route —
/// and it is grounded and shared, citing their word. A body never confirms one.
fn confirm(world: &Path, id: &str) -> ExitCode {
    use mnemorph_server::world::memory::{Proof, ground, proposed};
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: confirming a claim is the human's act, never a body's"
        ));
    }
    let owners: Vec<String> = std::fs::read_dir(world.join(".mnemorph/memory/creature"))
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default();
    for owner in owners {
        match proposed(world, &owner, id) {
            Ok(Some(claim)) => {
                return match ground(world, claim, &Proof::confirmed("the human's word")) {
                    Ok(_) => {
                        println!("confirmed {id}: {owner}'s claim, grounded and shared");
                        ExitCode::SUCCESS
                    }
                    Err(e) => refused(&format!("{id} could not be grounded: {e}")),
                };
            }
            Ok(None) => {}
            Err(e) => return refused(&format!("{owner}'s memory cannot be read: {e}")),
        }
    }
    refused(&format!(
        "no creature holds `{id}` as a proposed claim: `mnemorphd memory status` lists them"
    ))
}

/// `memory wake <note>`: the human restores a note the gate retired. A body never wakes one.
fn wake(world: &Path, id: &str) -> ExitCode {
    if let Ok(body) = std::env::var("MNEMORPH_BODY") {
        return refused(&format!(
            "{body} runs this: waking a note is the human's act, never a body's"
        ));
    }
    match mnemorph_server::world::memory::wake(world, id) {
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
        [verb] if verb == "score" => match mnemorph_server::world::challenge::scores(&world) {
            Ok(folded) => {
                let roster = mnemorph_server::world::Roster::load(&world);
                for line in score(&folded, &roster, &records) {
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
    // A proposal names the records' chain head, so it can be recomputed from them.
    let chain = records.chain();
    let head = match &chain {
        Ok(r) if r.holds() => r.head.as_ref().map(|h| h.digest.clone()),
        Ok(r) => {
            lines.push(format!("the records' chain does not hold: {r}"));
            None
        }
        Err(_) => None,
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
