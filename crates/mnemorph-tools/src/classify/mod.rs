//! The shell classifier: a command line in, a [`ToolClass`] or a refusal out.
//!
//! The command is parsed into a bash AST and every simple command in it — each segment of a
//! list or pipeline, each command substitution, each process substitution, each `sh -c` string —
//! is classified on its own; the command's class is the highest of them. Wrappers (`env`,
//! `sudo`, `xargs`, `nohup`, `timeout`, …) are unwrapped, git's global options skipped, command
//! names normalised (`/usr/bin/git`, `\rm`), braces expanded and `~` and `$HOME` resolved to the
//! real home before any path is judged.
//!
//! The classifier fails closed: what it cannot read — a parse error, a command name that is not
//! literal, an `eval` of a variable — is refused, never guessed. And arguments only tighten: a
//! simple command's class is the highest class of any prefix of its arguments, so no argument
//! can lower what the words before it already earned.

mod commands;
pub use commands::remote_name;
pub(crate) mod paths;
mod words;

use std::path::{Path, PathBuf};

use brush_parser::ast::{
    AndOr, Command, CommandPrefixOrSuffixItem, CompoundCommand, CompoundList, ExtendedTestExpr,
    IoFileRedirectKind, IoFileRedirectTarget, IoRedirect, Pipeline, Program, RedirectList,
    SimpleCommand,
};
use brush_parser::{Parser, ParserOptions};
use mnemorph_races::{OutwardKind, ToolClass};

use paths::{Act, locate};
use words::{Lit, expand};

/// Where a command runs: the world it may change, the human's home and the working directory.
#[derive(Debug, Clone)]
pub struct Place {
    /// The world's root. Paths under it are inside; everything else is outside.
    pub world: PathBuf,
    /// The human's home, resolved to its real path (the shell expands `~` before anyone looks).
    pub home: PathBuf,
    /// The directory the command starts in.
    pub cwd: PathBuf,
    /// The directories outside the world a deploy may write: a write anywhere else outside is
    /// no post's act.
    pub roots: Vec<PathBuf>,
}

impl Place {
    /// A place whose world and working directory are the same root.
    pub fn new(world: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Self {
        let world = world.into();
        Self {
            cwd: world.clone(),
            world,
            home: home.into(),
            roots: Vec::new(),
        }
    }

    /// The directories outside the world a deploy may write.
    #[must_use]
    pub fn with_roots(mut self, roots: impl IntoIterator<Item = PathBuf>) -> Self {
        self.roots = roots.into_iter().collect();
        self
    }
}

/// The classifier's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The command may run, behind whatever its class requires.
    Run {
        /// The highest class of any simple command in it.
        class: ToolClass,
        /// Why the class is above `read`, one line per reason.
        why: Vec<String>,
        /// The paths outside the world it writes or deletes, as the classifier located them —
        /// what an approval may open in the sandbox, and nothing more.
        outside: Vec<PathBuf>,
    },
    /// The command never runs unless the human's configuration turns its rule into a question
    /// (and the rule is not on the floor).
    Refuse {
        /// The guard rule that refused it.
        rule: Rule,
        /// The first reason found.
        reason: String,
        /// The class the command has besides the refusal: what it runs at if the human's
        /// configuration turns the rule into a question and the human approves.
        class: ToolClass,
    },
}

/// The guard's rules, by name. A configuration may turn a rule into "ask the human" instead of
/// "refuse" — except the floor, which no configuration loosens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rule {
    /// The command, a word in it, or a nested string cannot be read.
    Unreadable,
    /// A command name, `eval` or `sh -c` string known only at run time.
    NotLiteral,
    /// `sed -i`, `perl -i`: in-place edits that fail silently; the edit tool says where.
    InPlaceEdit,
    /// A password manager, the keychain, `gpg` secrets, a printed token.
    SecretStore,
    /// A credential path (`~/.ssh`, `~/.aws`, …) named by the command.
    Credential,
    /// A download piped into an interpreter.
    RemoteCode,
    /// Writing a device file (a disk). Floor.
    Device,
    /// Disk and power tools: `mkfs`, `fdisk`, `diskutil erase`, `shutdown`. Floor.
    MachineTool,
    /// Changing `/`, a top-level directory, or an ancestor of the world or the home. Floor.
    RootPath,
    /// A function that calls itself in the background: a fork bomb. Floor.
    ForkBomb,
    /// Writing or removing a Mind under `.mnemorph/minds/`: the human's alone. Floor.
    Mind,
}

impl Rule {
    /// Every rule.
    pub const ALL: [Rule; 11] = [
        Rule::Unreadable,
        Rule::NotLiteral,
        Rule::InPlaceEdit,
        Rule::SecretStore,
        Rule::Credential,
        Rule::RemoteCode,
        Rule::Device,
        Rule::MachineTool,
        Rule::RootPath,
        Rule::ForkBomb,
        Rule::Mind,
    ];

    /// The name a configuration uses.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Rule::Unreadable => "unreadable",
            Rule::NotLiteral => "not-literal",
            Rule::InPlaceEdit => "in-place-edit",
            Rule::SecretStore => "secret-store",
            Rule::Credential => "credential",
            Rule::RemoteCode => "remote-code",
            Rule::Device => "device",
            Rule::MachineTool => "machine-tool",
            Rule::RootPath => "root-path",
            Rule::ForkBomb => "fork-bomb",
            Rule::Mind => "mind",
        }
    }

    /// The rule a configuration names.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Rule> {
        Rule::ALL.into_iter().find(|r| r.name() == name)
    }

    /// On the floor: no configuration turns it into a question.
    #[must_use]
    pub fn floor(self) -> bool {
        matches!(
            self,
            Rule::Device | Rule::MachineTool | Rule::RootPath | Rule::ForkBomb | Rule::Mind
        )
    }
}

impl Verdict {
    /// The class, or `None` when refused.
    #[must_use]
    pub fn class(&self) -> Option<ToolClass> {
        match self {
            Self::Run { class, .. } => Some(*class),
            Self::Refuse { .. } => None,
        }
    }
}

/// Classify one command line run at `place`.
#[must_use]
pub fn classify(command: &str, place: &Place) -> Verdict {
    classify_acts(command, place).0
}

/// Classify one command line run at `place`, with the kinds of outward act it does — what a
/// Kijin's post is checked against. A command whose class is `outward` always names at least
/// one kind: one the classifier cannot name is [`OutwardKind::Other`].
#[must_use]
pub fn classify_acts(command: &str, place: &Place) -> (Verdict, Vec<OutwardKind>) {
    let (verdict, acts, _) = classify_reach(command, place);
    (verdict, acts)
}

/// A branch or tag push the classifier read as a post's act: the repository it runs in and the
/// remote it names, whose URL the gate resolves before the human is asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Push {
    /// The repository's directory.
    pub dir: PathBuf,
    /// The configured remote's name.
    pub remote: String,
}

/// [`classify_acts`], with the pushes whose remotes the gate resolves.
#[must_use]
pub fn classify_reach(command: &str, place: &Place) -> (Verdict, Vec<OutwardKind>, Vec<Push>) {
    let mut walker = Walker {
        place,
        cwd: Some(place.cwd.clone()),
        depth: 0,
        tainted: false,
    };
    let mut tally = Tally::default();
    walker.source(command, &mut tally);
    let mut acts = std::mem::take(&mut tally.acts);
    if tally.class == Some(ToolClass::Outward) && acts.is_empty() {
        acts.push(OutwardKind::Other);
    }
    acts.sort_unstable();
    let pushes = std::mem::take(&mut tally.pushes);
    (tally.verdict(), acts, pushes)
}

/// How far `sh -c` and `eval` strings nest before the classifier stops reading and refuses.
const MAX_DEPTH: u8 = 8;

/// The order of the four classes a command can have.
pub(crate) fn rank(class: ToolClass) -> u8 {
    match class {
        ToolClass::Read => 0,
        ToolClass::Write => 1,
        ToolClass::Outward => 2,
        // Dispatch and seal are harness tools, never shell; ranked high so a stray one tightens.
        ToolClass::Destructive | ToolClass::Dispatch | ToolClass::Seal => 3,
    }
}

/// What the walk found so far: a class that only rises, and the first refusal.
#[derive(Debug, Default)]
pub(crate) struct Tally {
    class: Option<ToolClass>,
    why: Vec<String>,
    refused: Option<(Rule, String)>,
    pub(crate) outside: Vec<PathBuf>,
    /// The kinds of outward act found.
    acts: Vec<OutwardKind>,
    /// The pushes read as a post's act.
    pub(crate) pushes: Vec<Push>,
}

impl Tally {
    /// Raise the class. An outward raise that names no kind is [`OutwardKind::Other`]: an act
    /// the classifier cannot name is held by no post.
    pub(crate) fn raise(&mut self, class: ToolClass, why: impl Into<String>) {
        if class == ToolClass::Outward {
            self.act(OutwardKind::Other);
        }
        self.lift(class, why);
    }

    /// Raise the class for an outward act of a known kind.
    pub(crate) fn reach(&mut self, kind: OutwardKind, class: ToolClass, why: impl Into<String>) {
        self.act(kind);
        self.lift(class, why);
    }

    fn act(&mut self, kind: OutwardKind) {
        if !self.acts.contains(&kind) {
            self.acts.push(kind);
        }
    }

    fn lift(&mut self, class: ToolClass, why: impl Into<String>) {
        if class != ToolClass::Read {
            self.why.push(why.into());
        }
        if self.class.is_none_or(|c| rank(class) > rank(c)) {
            self.class = Some(class);
        }
    }

    pub(crate) fn refuse(&mut self, rule: Rule, reason: impl Into<String>) {
        // The first refusal stands, except that a floor rule displaces one the human may ask:
        // no co-occurring rule can turn a floor refusal into a question.
        let displaces = self
            .refused
            .as_ref()
            .is_some_and(|(r, _)| !r.floor() && rule.floor());
        if self.refused.is_none() || displaces {
            self.refused = Some((rule, reason.into()));
        }
    }

    pub(crate) fn absorb(&mut self, other: Self) {
        if let Some((rule, r)) = other.refused {
            self.refuse(rule, r);
        }
        if let Some(c) = other.class
            && self.class.is_none_or(|mine| rank(c) > rank(mine))
        {
            self.class = Some(c);
        }
        for kind in other.acts {
            self.act(kind);
        }
        self.pushes.extend(other.pushes);
        for p in other.outside {
            if !self.outside.contains(&p) {
                self.outside.push(p);
            }
        }
        for w in other.why {
            if !self.why.contains(&w) {
                self.why.push(w);
            }
        }
    }

    fn verdict(self) -> Verdict {
        match self.refused {
            Some((rule, reason)) => Verdict::Refuse {
                rule,
                reason,
                class: self.class.unwrap_or(ToolClass::Read),
            },
            None => Verdict::Run {
                class: self.class.unwrap_or(ToolClass::Read),
                why: self.why,
                outside: self.outside,
            },
        }
    }
}

/// What a classified simple command was, for the pipeline rule (`curl … | sh`).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Kind {
    /// It fetches from the network.
    pub(crate) fetches: bool,
    /// It is an interpreter running code read from its standard input.
    pub(crate) runs_stdin: bool,
}

pub(crate) struct Walker<'p> {
    pub(crate) place: &'p Place,
    /// The working directory; `None` once a `cd` went somewhere the classifier cannot read.
    pub(crate) cwd: Option<PathBuf>,
    depth: u8,
    /// Something earlier in the command, or a prefix of this one, changed what a later command
    /// runs with: a variable assigned or exported, a function defined, git's config written, a
    /// script sourced, the directory changed. Every outward act after it is no post's.
    pub(crate) tainted: bool,
}

impl Walker<'_> {
    /// Parse and walk a piece of shell source: the command line, or an `sh -c` / `eval` string.
    pub(crate) fn source(&mut self, src: &str, t: &mut Tally) {
        if self.depth >= MAX_DEPTH {
            t.refuse(
                Rule::Unreadable,
                "shell strings nest deeper than the classifier reads",
            );
            return;
        }
        let program = match parse(src) {
            Ok(p) => p,
            Err(e) => {
                t.refuse(
                    Rule::Unreadable,
                    format!("the command does not parse as shell: {e}"),
                );
                return;
            }
        };
        self.depth += 1;
        self.program(&program, t);
        self.depth -= 1;
    }

    fn program(&mut self, p: &Program, t: &mut Tally) {
        for list in &p.complete_commands {
            self.list(list, t);
        }
    }

    fn list(&mut self, list: &CompoundList, t: &mut Tally) {
        for item in &list.0 {
            let and_or = &item.0;
            self.pipeline(&and_or.first, t);
            for next in &and_or.additional {
                match next {
                    AndOr::And(p) | AndOr::Or(p) => self.pipeline(p, t),
                }
            }
        }
    }

    fn pipeline(&mut self, p: &Pipeline, t: &mut Tally) {
        // Every stage of a real pipeline runs in its own subshell: a `cd` there stays there.
        let saved = (p.seq.len() > 1).then(|| self.cwd.clone());
        let mut fetched = false;
        for cmd in &p.seq {
            let kind = self.command(cmd, t);
            if fetched && kind.runs_stdin {
                t.refuse(
                    Rule::RemoteCode,
                    "runs code fetched from the network (a download piped into an interpreter)",
                );
            }
            fetched |= kind.fetches;
        }
        if let Some(cwd) = saved {
            self.cwd = cwd;
        }
    }

    fn command(&mut self, cmd: &Command, t: &mut Tally) -> Kind {
        match cmd {
            Command::Simple(s) => self.simple(s, t),
            Command::Compound(c, redirects) => {
                self.compound(c, t);
                self.redirects(redirects.as_ref(), t);
                Kind::default()
            }
            Command::Function(f) => {
                self.tainted = true;
                let mut names = Vec::new();
                collect_names(&f.body.0, &mut names);
                if names.iter().any(|n| *n == f.fname.value) {
                    t.refuse(
                        Rule::ForkBomb,
                        format!(
                            "the function `{}` calls itself (the shape of a fork bomb)",
                            f.fname.value
                        ),
                    );
                }
                self.compound(&f.body.0, t);
                self.redirects(f.body.1.as_ref(), t);
                Kind::default()
            }
            Command::ExtendedTest(e, redirects) => {
                self.test_expr(&e.expr, t);
                self.redirects(redirects.as_ref(), t);
                Kind::default()
            }
        }
    }

    fn compound(&mut self, c: &CompoundCommand, t: &mut Tally) {
        match c {
            // An arithmetic assignment or a loop variable changes what later commands run with.
            CompoundCommand::Arithmetic(_) | CompoundCommand::ArithmeticForClause(_) => {
                self.tainted = true;
            }
            CompoundCommand::BraceGroup(b) => self.list(&b.list, t),
            CompoundCommand::Subshell(s) => {
                let saved = self.cwd.clone();
                self.list(&s.list, t);
                self.cwd = saved;
            }
            CompoundCommand::ForClause(f) => {
                self.tainted = true;
                for w in f.values.iter().flatten() {
                    self.word_subs(&w.value, t);
                }
                self.list(&f.body.list, t);
            }
            CompoundCommand::CaseClause(c) => {
                self.word_subs(&c.value.value, t);
                for item in &c.cases {
                    if let Some(body) = &item.cmd {
                        self.list(body, t);
                    }
                }
            }
            CompoundCommand::IfClause(i) => {
                self.list(&i.condition, t);
                self.list(&i.then, t);
                for e in i.elses.iter().flatten() {
                    if let Some(c) = &e.condition {
                        self.list(c, t);
                    }
                    self.list(&e.body, t);
                }
            }
            CompoundCommand::WhileClause(w) | CompoundCommand::UntilClause(w) => {
                self.list(&w.0, t);
                self.list(&w.1.list, t);
            }
            CompoundCommand::Coprocess(c) => {
                self.command(&c.body, t);
            }
        }
    }

    fn test_expr(&mut self, e: &ExtendedTestExpr, t: &mut Tally) {
        match e {
            ExtendedTestExpr::And(a, b) | ExtendedTestExpr::Or(a, b) => {
                self.test_expr(a, t);
                self.test_expr(b, t);
            }
            ExtendedTestExpr::Not(a) | ExtendedTestExpr::Parenthesized(a) => self.test_expr(a, t),
            ExtendedTestExpr::UnaryTest(_, w) => self.word_subs(&w.value, t),
            ExtendedTestExpr::BinaryTest(_, a, b) => {
                self.word_subs(&a.value, t);
                self.word_subs(&b.value, t);
            }
        }
    }

    fn simple(&mut self, s: &SimpleCommand, t: &mut Tally) -> Kind {
        let mut argv: Vec<Lit> = Vec::new();
        let mut redirects: Vec<&IoRedirect> = Vec::new();
        let prefix = s.prefix.iter().flat_map(|p| p.0.iter()).map(|i| (i, false));
        let suffix = s.suffix.iter().flat_map(|x| x.0.iter()).map(|i| (i, true));
        let mut words: Vec<&str> = s.word_or_name.iter().map(|w| w.value.as_str()).collect();
        for (item, after_name) in prefix.chain(suffix) {
            match item {
                CommandPrefixOrSuffixItem::IoRedirect(r) => redirects.push(r),
                CommandPrefixOrSuffixItem::Word(w) => words.push(&w.value),
                // After the command name `x=y` is an argument (`dd of=…`, `make CC=…`).
                CommandPrefixOrSuffixItem::AssignmentWord(_, w) if after_name => {
                    words.push(&w.value);
                }
                CommandPrefixOrSuffixItem::AssignmentWord(_, w) => {
                    // `GIT_SSH_COMMAND=… git fetch`: the variable changes what the command runs.
                    self.tainted = true;
                    self.word_subs(&w.value, t);
                }
                CommandPrefixOrSuffixItem::ProcessSubstitution(_, sub) => {
                    let saved = self.cwd.clone();
                    self.list(&sub.list, t);
                    self.cwd = saved;
                }
            }
        }
        // The command word comes first even when the parser filed it under the suffix.
        for w in words {
            match expand(w, &self.place.home) {
                Ok(lits) => argv.extend(lits),
                Err(e) => {
                    t.refuse(Rule::Unreadable, e);
                    return Kind::default();
                }
            }
        }
        for lit in &argv {
            for sub in &lit.subs {
                self.source(sub, t);
            }
        }
        for r in redirects {
            self.redirect(r, t);
        }
        if argv.is_empty() {
            return Kind::default();
        }
        if argv[0].text.as_deref() == Some("cd") {
            self.tainted = true;
            self.cd(&argv[1..]);
            return Kind::default();
        }
        // Arguments only tighten: the class is the highest over every prefix of the arguments.
        let mut kind = Kind::default();
        // The acts are the whole command's: a prefix's (`gh` alone) is not what runs.
        for k in 1..=argv.len() {
            let mut part = Tally::default();
            kind = commands::classify(self, &argv[..k], &mut part);
            if k < argv.len() {
                part.acts.clear();
                part.pushes.clear();
            } else if self.tainted && !part.acts.is_empty() {
                part.acts = vec![OutwardKind::Other];
                part.pushes.clear();
            }
            t.absorb(part);
        }
        kind
    }

    fn cd(&mut self, args: &[Lit]) {
        let target = args
            .iter()
            .find(|a| a.text.as_deref().is_none_or(|s| !s.starts_with('-')));
        self.cwd = match target {
            None => Some(self.place.home.clone()),
            Some(Lit {
                text: Some(dir), ..
            }) => {
                let base = self.cwd.clone();
                paths::join(base.as_deref(), dir)
            }
            Some(_) => None,
        };
    }

    fn redirects(&mut self, list: Option<&RedirectList>, t: &mut Tally) {
        for r in list.iter().flat_map(|l| l.0.iter()) {
            self.redirect(r, t);
        }
    }

    fn redirect(&mut self, r: &IoRedirect, t: &mut Tally) {
        match r {
            IoRedirect::File(_, kind, target) => match target {
                IoFileRedirectTarget::Filename(w) => {
                    let act = match kind {
                        IoFileRedirectKind::Read => Act::Read,
                        IoFileRedirectKind::DuplicateInput
                        | IoFileRedirectKind::DuplicateOutput => {
                            return;
                        }
                        _ => Act::Modify,
                    };
                    self.path_word(&w.value, act, "a redirect", t);
                }
                IoFileRedirectTarget::ProcessSubstitution(_, sub) => {
                    let saved = self.cwd.clone();
                    self.list(&sub.list, t);
                    self.cwd = saved;
                }
                IoFileRedirectTarget::Fd(_) | IoFileRedirectTarget::Duplicate(_) => {}
            },
            IoRedirect::HereDocument(_, doc) => {
                if doc.requires_expansion {
                    self.word_subs(&doc.doc.value, t);
                }
            }
            IoRedirect::HereString(_, w) => self.word_subs(&w.value, t),
            IoRedirect::OutputAndError(w, _) => {
                self.path_word(&w.value, Act::Modify, "a redirect", t);
            }
        }
    }

    /// Classify the command substitutions inside a word that is not itself judged.
    fn word_subs(&mut self, raw: &str, t: &mut Tally) {
        match expand(raw, &self.place.home) {
            Ok(lits) => {
                for sub in lits.iter().flat_map(|l| l.subs.iter()) {
                    self.source(sub, t);
                }
            }
            Err(e) => t.refuse(Rule::Unreadable, e),
        }
    }

    fn path_word(&mut self, raw: &str, act: Act, what: &str, t: &mut Tally) {
        match expand(raw, &self.place.home) {
            Ok(lits) => {
                for lit in lits {
                    for sub in &lit.subs {
                        self.source(sub, t);
                    }
                    self.touch(&lit, act, what, t);
                }
            }
            Err(e) => t.refuse(Rule::Unreadable, e),
        }
    }

    /// Judge one path the command reads, modifies or deletes.
    pub(crate) fn touch(&self, lit: &Lit, act: Act, what: &str, t: &mut Tally) {
        let spot = locate(lit.text.as_deref(), self.cwd.as_deref(), self.place);
        paths::judge(&spot, act, what, self.place, t);
    }
}

fn parse(src: &str) -> Result<Program, String> {
    let options = ParserOptions::default();
    let mut parser = Parser::new(src.as_bytes(), &options);
    parser.parse_program().map_err(|e| e.to_string())
}

/// The literal command names a compound command runs, for the fork-bomb check.
fn collect_names<'a>(c: &'a CompoundCommand, out: &mut Vec<&'a str>) {
    fn list<'a>(l: &'a CompoundList, out: &mut Vec<&'a str>) {
        for item in &l.0 {
            let ao = &item.0;
            for p in std::iter::once(&ao.first).chain(ao.additional.iter().map(|a| match a {
                AndOr::And(p) | AndOr::Or(p) => p,
            })) {
                for cmd in &p.seq {
                    match cmd {
                        Command::Simple(s) => {
                            if let Some(w) = &s.word_or_name {
                                out.push(&w.value);
                            }
                        }
                        Command::Compound(c, _) => collect_names(c, out),
                        Command::Function(_) | Command::ExtendedTest(..) => {}
                    }
                }
            }
        }
    }
    match c {
        CompoundCommand::BraceGroup(b) => list(&b.list, out),
        CompoundCommand::Subshell(s) => list(&s.list, out),
        CompoundCommand::ForClause(f) => list(&f.body.list, out),
        CompoundCommand::ArithmeticForClause(f) => list(&f.body.list, out),
        CompoundCommand::WhileClause(w) | CompoundCommand::UntilClause(w) => {
            list(&w.0, out);
            list(&w.1.list, out);
        }
        CompoundCommand::IfClause(i) => {
            list(&i.condition, out);
            list(&i.then, out);
            for e in i.elses.iter().flatten() {
                list(&e.body, out);
            }
        }
        CompoundCommand::CaseClause(c) => {
            for item in &c.cases {
                if let Some(b) = &item.cmd {
                    list(b, out);
                }
            }
        }
        CompoundCommand::Arithmetic(_) | CompoundCommand::Coprocess(_) => {}
    }
}

/// The last path segment of a command name: `/usr/bin/git` is `git`.
pub(crate) fn base_name(name: &str) -> &str {
    Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(name)
}
