//! `mnemorphd` — the Mnemorph core. It speaks the client protocol on stdio; the CLI and the board
//! are its clients, and decide nothing.

use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const HELP: &str = "mnemorphd — the Mnemorph core

Usage: mnemorphd [--world <dir>]
       mnemorphd onto owner <path>... [--world <dir>]
       mnemorphd onto check [--world <dir>]
       mnemorphd onto settle <fact-id> [--world <dir>]
       mnemorphd housekeep [--heal] [--world <dir>]
       mnemorphd relief <name> [--apply] [--world <dir>]
       mnemorphd memory status [--world <dir>] [--json]
       mnemorphd memory wake <note> [--world <dir>]
       mnemorphd q list [--all] | show <id> | add <ask> [--for <body>] [--rank <n>]
                 | claim <id> [--as <body>] | touch <id> <note> | requeue <id>
                 | close <id> [--drop]   [--world <dir>] [--json]

Speaks the client protocol (JSON-RPC 2.0, one message per line) on stdin and stdout. The
`mnemorph` CLI starts it; run it by hand only to test a client.

`onto owner` names each path's owner, the rule that gave it and the claim that matched, read
from the creature docs; it needs no key and opens no session. A path is relative to the
current directory, or absolute. `onto check` prints `@S PASS|FAIL <n> findings`, then each
finding's severity, code, place and message; it fails on a critical or high finding, or a doc
that does not read.

`memory status` names, per creature, every note event whose author or proof does not resolve —
a hand edit, a bad merge, an import that skipped its checks. Recall ignores such an event and
the creature's memory writes stay frozen until the event is removed or re-proved; it exits 1
while any is found.

`q` is the shared work queue's door for the human, who stands over every body: a refusal exits 2
and says why.

Options:
  --world <dir>   the world to work in (default: the current directory)
  -h, --help      this help
  -V, --version   the version
";

/// The version and the commit it was built from: `0.0.0 (a1b2c3d4e5)`.
const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("MNEMORPH_COMMIT"),
    ")"
);

fn main() -> ExitCode {
    let mut world = None;
    let all: Vec<String> = std::env::args().skip(1).collect();
    if all.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if all.iter().any(|a| a == "-V" || a == "--version") {
        println!("mnemorphd {VERSION}");
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
    let mut args = all.into_iter();
    // Help and the version answer before any world is looked for.
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("mnemorphd {VERSION}");
                return ExitCode::SUCCESS;
            }
            "--world" => match args.next() {
                Some(dir) => world = Some(PathBuf::from(dir)),
                None => return fail("--world needs a directory"),
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
    match mnemorph_server::serve(
        input,
        std::io::stdout(),
        VERSION,
        mnemorph_server::opener(world),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e),
    }
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
            f.at,
            f.message
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
    let mut it = args.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--heal" => apply = true,
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
                Some(id) => verb = Some(Some(id.clone())),
                None => return refused(&"wake needs the note's id"),
            },
            other => return refused(&format!("unknown argument `{other}`; see --help")),
        }
    }
    let Some(verb) = verb else {
        return refused(&"memory needs a verb: status, or wake <note>");
    };
    let Ok(cwd) = std::env::current_dir() else {
        return refused(&"the current directory cannot be read");
    };
    let world = world.map_or_else(|| cwd.clone(), |w| absolute(&cwd, &w));
    if let Some(id) = verb {
        return wake(&world, &id);
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
                    "retired, kept dormant: {id} — {text} (undo: mnemorph memory wake {id})"
                ));
            }
        }
    }
    if json {
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
                })
            })
            .collect();
        println!("{}", serde_json::Value::Array(rows));
    } else if rows.is_empty() {
        println!("no creature has memory here yet");
    } else {
        println!("{}", standing(&world));
        for (name, found, seen) in &rows {
            print_memory(name, found, seen);
        }
    }
    if frozen {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
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
    use mnemorph_server::world::memory::{Status, Tier, fold, read};
    let count = |tier: &Tier, status: Status| {
        read(world, tier).map_or(0, |r| {
            fold(&r.rows)
                .values()
                .filter(|n| n.status == status)
                .count()
        })
    };
    let shared = count(&Tier::Shared, Status::Grounded);
    let (mut proposed, mut grounded) = (0, 0);
    if let Ok(rd) = std::fs::read_dir(world.join(".mnemorph/memory/creature")) {
        for e in rd.filter_map(Result::ok) {
            if let Some(name) = e
                .path()
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
            {
                let tier = Tier::Personal(name);
                proposed += count(&tier, Status::Proposed);
                grounded += count(&tier, Status::Grounded);
            }
        }
    }
    format!(
        "memory: {shared} note(s) shared and grounded; personal: {proposed} proposed, {grounded} grounded"
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
