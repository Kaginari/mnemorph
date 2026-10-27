//! The report: what a body sends back.

use std::borrow::Cow;
use std::fmt::Write as _;

use crate::{fnv, forge::Forge, hole::Hole, tag};

/// A report longer than this travels as a courier envelope: the full text in a file, the
/// reader receives [`Report::envelope`].
pub const COURIER_BYTES: usize = 600;

/// How many lines of findings and verdicts a courier envelope carries inline.
const DIGEST_LINES: usize = 8;

/// The word that opens a status: what every fold reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Word {
    /// The work landed.
    Done,
    /// A judgement passed.
    Pass,
    /// It did not.
    Fail,
    /// One hop up: the dispatcher decides.
    Escalate,
    /// An honest stop at a budget; resumable.
    Checkpoint,
    /// The human said no.
    Denied,
    /// The human cut the run.
    Interrupted,
    /// A dry run.
    Dry,
    /// Waiting on the human.
    Suspended,
    /// Any other word, as written.
    Other(String),
}

impl Word {
    fn read(word: &str) -> Word {
        match word {
            "DONE" => Word::Done,
            "PASS" => Word::Pass,
            "FAIL" => Word::Fail,
            "ESCALATE" => Word::Escalate,
            "CHECKPOINT" => Word::Checkpoint,
            "DENIED" => Word::Denied,
            "INTERRUPTED" => Word::Interrupted,
            "DRY" => Word::Dry,
            "SUSPENDED" => Word::Suspended,
            other => Word::Other(other.to_owned()),
        }
    }
}

/// The three homes of the unsaid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UnsaidKind {
    /// Institutional knowledge: a rule, definition or decision.
    Law,
    /// Tribal knowledge: what the colony knows and rarely writes down.
    Colony,
    /// Domain context: what the numbers and entities mean in a zone.
    Territory,
}

impl UnsaidKind {
    /// Its word on the wire: `law`, `colony`, `territory`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            UnsaidKind::Law => "law",
            UnsaidKind::Colony => "colony",
            UnsaidKind::Territory => "territory",
        }
    }

    fn from_word(word: &str) -> Option<Self> {
        match word {
            "law" => Some(UnsaidKind::Law),
            "colony" => Some(UnsaidKind::Colony),
            "territory" => Some(UnsaidKind::Territory),
            _ => None,
        }
    }
}

/// One `@U` line: knowledge that was in a head and nowhere on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsaid {
    /// Its home; `None` when the body did not say.
    pub kind: Option<UnsaidKind>,
    /// The knowledge.
    pub text: String,
}

/// The word a finding's renderer puts before or after a claim that would otherwise read as an
/// id, a location, a witness or a record. The parser drops one at each end.
const GUARD: &str = "—";

/// One `@F #<id> <file:line> <claim> witness:<path> rec:<path>` finding. Every part but the claim
/// is optional; the id lets a later fix, contest or seal point at it, the witness is what makes a
/// Dark Elf's finding count, and the record holds its trace, severity and fix. The tail's order
/// is fixed: `witness:` then `rec:`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Finding {
    /// `#id`, without the `#`. An empty id is no id. A Dark Elf's id is its root-cause
    /// [`fingerprint`].
    pub id: Option<String>,
    /// `file:line` or `file:line-line`.
    pub at: Option<String>,
    /// The claim; its whitespace is normalised to single spaces.
    pub claim: String,
    /// `witness:<path>`: a failing proof or a scanner output.
    pub witness: Option<String>,
    /// `rec:<path>`: the finding's record.
    pub rec: Option<String>,
}

impl Finding {
    /// Reads a finding's payload. Anything it cannot place stays in the claim.
    #[must_use]
    pub fn parse(payload: &str) -> Self {
        let mut words: Vec<&str> = payload.split_whitespace().collect();
        let mut f = Finding::default();
        if let Some(first) = words.first()
            && let Some(id) = first.strip_prefix('#')
            && !id.is_empty()
        {
            f.id = Some(id.to_owned());
            words.remove(0);
        }
        if let Some(first) = words.first()
            && is_location(first)
        {
            f.at = Some((*first).to_owned());
            words.remove(0);
        }
        if words.first() == Some(&GUARD) {
            words.remove(0);
        }
        // The tail, from the end: a fence ends it; otherwise `rec:` first, then `witness:`, and
        // a fence between a lone record and the claim.
        if words.last() == Some(&GUARD) {
            words.pop();
        } else {
            f.rec = take_tail(&mut words, "rec:");
            if f.rec.is_some() && words.last() == Some(&GUARD) {
                words.pop();
            } else {
                f.witness = take_tail(&mut words, "witness:");
            }
        }
        f.claim = words.join(" ");
        f
    }

    /// The finding's content address: ten hex digits of FNV-1a over its location, claim and
    /// witness, so a relayed finding keeps the same address at every hop. Use it as the id of a
    /// finding that has none. It holds the line, so it is never a Dark Elf's id: that is its
    /// [`fingerprint`].
    #[must_use]
    pub fn address(&self) -> String {
        let claim = self.claim.split_whitespace().collect::<Vec<_>>().join(" ");
        let key = framed(&[
            self.at.as_deref().unwrap_or(""),
            &claim,
            self.witness.as_deref().unwrap_or(""),
        ]);
        format!("{:016x}", fnv::fnv64a(key.as_bytes()))[..10].to_owned()
    }

    /// Whether this finding carries the Dark Elf's contract on the wire: a fingerprint-shaped id,
    /// a witness and a record. The shape alone proves nothing: [`Report::dark_elf`] recomputes
    /// the id from the record.
    #[must_use]
    pub fn has_dark_elf_parts(&self) -> bool {
        self.id.as_deref().is_some_and(is_fingerprint)
            && self.witness.as_deref().is_some_and(|w| !w.is_empty())
            && self.rec.as_deref().is_some_and(|r| !r.is_empty())
    }

    fn render(&self) -> String {
        let id = self.id.as_deref().filter(|i| !i.is_empty());
        // Folded first, so a break that becomes a space cannot slip a word past the fence.
        let folded = one_line(&self.claim);
        let claim: Vec<&str> = folded.split_whitespace().collect();
        let witness = self.witness.as_ref().map(|w| format!("witness:{w}"));
        let rec = self.rec.as_ref().map(|r| format!("rec:{r}"));
        // With no claim, the first word of the tail stands where a location is read.
        let first = claim
            .first()
            .copied()
            .or(witness.as_deref())
            .or(rec.as_deref());
        let last = claim.last().copied();
        // A claim word that the parser would take for something else is fenced with GUARD.
        let lead = first.is_some_and(|w| {
            w == GUARD
                || (self.at.is_none()
                    && (is_location(w) || (id.is_none() && w.len() > 1 && w.starts_with('#'))))
        });
        // With a witness, the parser stops at it: the claim's last word is safe. Without one,
        // a last word the parser would take as a witness, a record or a fence is fenced.
        let trail = self.witness.is_none()
            && last.is_some_and(|w| {
                w == GUARD || opens(w, "witness:") || (self.rec.is_none() && opens(w, "rec:"))
            });
        let mut out = String::new();
        if let Some(id) = id {
            let _ = write!(out, "#{id} ");
        }
        if let Some(at) = &self.at {
            let _ = write!(out, "{at} ");
        }
        if lead {
            let _ = write!(out, "{GUARD} ");
        }
        out.push_str(&claim.join(" "));
        if trail {
            let _ = write!(out, " {GUARD}");
        }
        for tail in [witness, rec].into_iter().flatten() {
            let _ = write!(out, " {tail}");
        }
        out.trim().to_owned()
    }
}

/// Whether `word` opens `field` with a non-empty value.
fn opens(word: &str, field: &str) -> bool {
    word.strip_prefix(field).is_some_and(|v| !v.is_empty())
}

/// Pops the last word when it opens `field` with a value, and gives the value.
fn take_tail(words: &mut Vec<&str>, field: &str) -> Option<String> {
    let value = words
        .last()?
        .strip_prefix(field)
        .filter(|v| !v.is_empty())?;
    let value = value.to_owned();
    words.pop();
    Some(value)
}

/// The fields framed for a hash: each as `<byte length>:<field>`, so no field's content can
/// move a boundary between fields.
fn framed(fields: &[&str]) -> String {
    fields.iter().fold(String::new(), |mut key, f| {
        let _ = write!(key, "{}:{f}", f.len());
        key
    })
}

/// Ten lowercase hex digits: the shape of a fingerprint.
fn is_fingerprint(id: &str) -> bool {
    id.len() == 10 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A Dark Elf finding's root cause, as its record states it: what its fingerprint is made of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootCause {
    /// The speciality's full name (`dark-elf-agent`).
    pub speciality: String,
    /// The playbook's class id.
    pub class: String,
    /// The file, repository-relative.
    pub file: String,
    /// The symbol, as the code names it.
    pub symbol: String,
}

impl RootCause {
    /// Its fingerprint, or `None` when a field is refused (see [`fingerprint`]).
    #[must_use]
    pub fn fingerprint(&self) -> Option<String> {
        fingerprint(&self.speciality, &self.class, &self.file, &self.symbol)
    }
}

/// A Dark Elf finding's root-cause fingerprint: ten hex digits, the first ten of the SHA-256 of
/// the four fields framed as `<byte length>:<field>` each, in the order speciality, class, file,
/// symbol. Each field is trimmed and its whitespace folded to single spaces, and none may be
/// empty. The file is normalised by [`repo_path`]; a file it refuses has no fingerprint. No line,
/// body or severity enters it, so the same weakness keeps its id across runs and after its line
/// moves.
#[must_use]
pub fn fingerprint(speciality: &str, class: &str, file: &str, symbol: &str) -> Option<String> {
    use sha2::{Digest, Sha256};
    let fold = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let (speciality, class, symbol) = (fold(speciality), fold(class), fold(symbol));
    let file = repo_path(file)?;
    if [&speciality, &class, &symbol].iter().any(|f| f.is_empty()) {
        return None;
    }
    let digest = Sha256::digest(framed(&[&speciality, &class, &file, &symbol]).as_bytes());
    Some(digest[..5].iter().fold(String::new(), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    }))
}

/// A fingerprint's file, normalised: trimmed, `\` read as `/`, leading `./` dropped. It is then
/// refused unless it is printable ASCII with no space, not absolute (no leading `/`, no `:` in its
/// first segment), and every segment is non-empty and neither `.` nor `..`.
#[must_use]
pub fn repo_path(file: &str) -> Option<String> {
    let file = file.trim().replace('\\', "/");
    let mut file = file.as_str();
    while let Some(rest) = file.strip_prefix("./") {
        file = rest;
    }
    let ascii = file.bytes().all(|b| b.is_ascii_graphic());
    let mut segments = file.split('/');
    let drive = segments.next().is_some_and(|first| first.contains(':'));
    let bad = file
        .split('/')
        .any(|s| s.is_empty() || s == "." || s == "..");
    (ascii && !drive && !bad).then(|| file.to_owned())
}

/// A Dark Elf finding whose id the harness recomputed from its record. Only
/// [`Report::dark_elf`] makes one, so whatever scores or seals findings can take nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proven {
    finding: Finding,
    root: RootCause,
}

impl Proven {
    /// The finding as the wire carried it; its id is its root cause's fingerprint.
    #[must_use]
    pub fn finding(&self) -> &Finding {
        &self.finding
    }

    /// The root cause its record states.
    #[must_use]
    pub fn root(&self) -> &RootCause {
        &self.root
    }
}

/// A Dark Elf's report as the harness reads it: its proven findings, and the rest of the report
/// with no finding left in it — every line that was not proven rides as a hole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DarkElfReport {
    /// The report; its `findings` are always empty.
    pub report: Report,
    proven: Vec<Proven>,
}

impl DarkElfReport {
    /// The findings whose ids match their records: the only ones that count.
    #[must_use]
    pub fn proven(&self) -> &[Proven] {
        &self.proven
    }
}

/// `file:line` or `file:line-line`, with a non-empty file part.
fn is_location(word: &str) -> bool {
    let Some((file, line)) = word.rsplit_once(':') else {
        return false;
    };
    let (start, end) = line.split_once('-').unwrap_or((line, "0"));
    !file.is_empty()
        && !start.is_empty()
        && start.bytes().all(|b| b.is_ascii_digit())
        && !end.is_empty()
        && end.bytes().all(|b| b.is_ascii_digit())
}

/// One `@$ <body>: <ask>` line: work ordered one layer down, for the next turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enqueue {
    /// The body ordered, lower-cased.
    pub body: String,
    /// What it is asked.
    pub ask: String,
}

impl Enqueue {
    /// Reads `<body>: <ask>`. The body is one word; anything else is prose, not an order.
    #[must_use]
    pub fn parse(rest: &str) -> Option<Self> {
        let (body, ask) = rest.split_once(':')?;
        let body = body.trim();
        if body.is_empty() || body.contains(char::is_whitespace) {
            return None;
        }
        Some(Enqueue {
            body: body.to_ascii_lowercase(),
            ask: ask.trim().to_owned(),
        })
    }
}

/// Where the full report of a courier envelope lies, and its hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Courier {
    /// `@R` — the file holding the full report.
    pub path: String,
    /// `@H` — its FNV-1a 64 hash, `fnv64:<16 hex>`.
    pub hash: String,
}

impl Courier {
    /// The `@H` value of a filed text.
    #[must_use]
    pub fn hash_of(filed: &str) -> String {
        format!("fnv64:{:016x}", fnv::fnv64a(filed.as_bytes()))
    }

    /// Whether `filed` is the text this envelope points at.
    #[must_use]
    pub fn matches(&self, filed: &str) -> bool {
        self.hash == Self::hash_of(filed)
    }
}

/// What a body sends back.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// `@S` — the status: a word (`DONE`, `PASS`, `FAIL`, …) and anything after it.
    pub status: String,
    /// `@?` — holes: what could not be settled, never guessed.
    pub holes: Vec<Hole>,
    /// `@U` — the unsaid.
    pub unsaid: Vec<Unsaid>,
    /// `@R` + `@H` — set on a courier envelope.
    pub courier: Option<Courier>,
    /// `@V` — verdicts.
    pub verdicts: Vec<String>,
    /// `@F` — findings.
    pub findings: Vec<Finding>,
    /// `@W` — paths written.
    pub written: Vec<String>,
    /// `@T` — tools offered (the toolbox manifest).
    pub tools: Vec<String>,
    /// `@$` — orders for the next turn.
    pub enqueues: Vec<Enqueue>,
    /// `@FORGE` — drafts riding up.
    pub forges: Vec<Forge>,
    /// Lines the parser did not recognise, kept in order.
    pub other: Vec<String>,
    /// `@E` — the byte length as read; set by the parsers only.
    pub bytes: Option<usize>,
}

/// The sections of an emitted report, in emit order.
const SECTIONS: usize = 11;
/// Indices of the sections a cut may shorten, in the order it shortens them: prose, `@T`,
/// `@FORGE`, `@W`, `@F`, `@V`.
const CUT_ORDER: [usize; 6] = [10, 8, 9, 7, 6, 5];
/// The index of the holes section, where a cut names itself.
const HOLES: usize = 1;

impl Report {
    /// Reads a report in the tagged form. `None` when no `@S` opened an answer: such a text is
    /// not on the wire. Prose around the envelope is kept in [`Report::other`].
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::read(text, false)
    }

    /// Reads a report that may use the raw pipe form (`S|PASS`, `F|path:ln|claim`) as well as
    /// the tagged one — only for a body whose commission said `wire:raw`.
    #[must_use]
    pub fn parse_raw(text: &str) -> Option<Self> {
        Self::read(text, true)
    }

    fn read(text: &str, raw: bool) -> Option<Self> {
        let mut r = Report::default();
        let (mut seen, mut closed) = (false, false);
        let (mut path, mut hash) = (None, None);
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let Some((t, rest)) = tag::split(line, raw) else {
                r.other.push(tag::unescape(line).to_owned());
                continue;
            };
            let kept = match t {
                // The first `@S` opens the answer and the first `@E` closes it: a later one is
                // prose, so text after a status — a child's answer under the harness's own
                // `@S` — can never overwrite it.
                "S" if !seen => {
                    r.status = rest;
                    seen = true;
                    true
                }
                "?" => {
                    r.holes.push(Hole::parse(&rest));
                    true
                }
                "U" => {
                    let (kind, text) = match rest.split_once(char::is_whitespace) {
                        Some((w, text)) if UnsaidKind::from_word(w).is_some() => {
                            (UnsaidKind::from_word(w), text.trim().to_owned())
                        }
                        _ if UnsaidKind::from_word(&rest).is_some() => {
                            (UnsaidKind::from_word(&rest), String::new())
                        }
                        _ => (None, rest),
                    };
                    r.unsaid.push(Unsaid { kind, text });
                    true
                }
                "R" => path.replace(rest).is_none(),
                "H" => hash.replace(rest).is_none(),
                "V" => push(&mut r.verdicts, rest),
                "F" => {
                    r.findings.push(Finding::parse(&rest));
                    true
                }
                "W" => push(&mut r.written, rest),
                "T" => push(&mut r.tools, rest),
                "$" => Enqueue::parse(&rest).map(|q| r.enqueues.push(q)).is_some(),
                "FORGE" => Forge::parse(&rest).map(|f| r.forges.push(f)).is_some(),
                "E" if !closed => {
                    r.bytes = rest.split_whitespace().next().and_then(|n| n.parse().ok());
                    closed = true;
                    true
                }
                _ => false,
            };
            if !kept {
                r.other.push(line.to_owned());
            }
        }
        match (path, hash) {
            (Some(path), Some(hash)) => r.courier = Some(Courier { path, hash }),
            (Some(path), None) => r.other.push(format!("@R {path}")),
            (None, Some(hash)) => r.other.push(format!("@H {hash}")),
            (None, None) => {}
        }
        seen.then_some(r)
    }

    /// Reads this report as a Dark Elf's. Each finding's record is looked up by its `rec:` path
    /// (`record` reads it; `None` when it is missing or broken) and its id recomputed from the
    /// root cause there. A finding is proven only when it has a witness and a record and its
    /// stated id is that fingerprint; any other becomes a hole — a suspicion, never a finding.
    #[must_use]
    pub fn dark_elf(mut self, mut record: impl FnMut(&str) -> Option<RootCause>) -> DarkElfReport {
        let mut proven = Vec::new();
        for f in std::mem::take(&mut self.findings) {
            let why = if f.has_dark_elf_parts() {
                match f.rec.as_deref().and_then(&mut record) {
                    None => "no record",
                    Some(root) if root.fingerprint() == f.id => {
                        proven.push(Proven { finding: f, root });
                        continue;
                    }
                    Some(_) => "forged id",
                }
            } else {
                "unproven"
            };
            self.holes
                .push(Hole::Other(format!("{why}: {}", f.render())));
        }
        DarkElfReport {
            report: self,
            proven,
        }
    }

    /// Whether `text` is a whole report: on the wire, and its `@E` equal to its byte length.
    #[must_use]
    pub fn is_whole(text: &str) -> bool {
        Self::parse(text).and_then(|r| r.bytes) == Some(text.len())
    }

    /// The first word of the status, the word every fold reads.
    #[must_use]
    pub fn word(&self) -> Word {
        Word::read(self.status.split_whitespace().next().unwrap_or(""))
    }

    /// The envelope lines by section, without `@E`. Every field is folded to one line, and a
    /// prose line that would read as a tag rides escaped, so no text can open a line of its own.
    fn sections(&self) -> [Vec<String>; SECTIONS] {
        let status = format!("@S {}", kept(self.status.trim()).trim());
        let unsaid = |u: &Unsaid| match u.kind {
            Some(k) => format!("@U {} {}", k.word(), kept(&u.text)),
            None => format!("@U {}", kept(&u.text)),
        };
        let courier = self.courier.iter().flat_map(|c| {
            [
                format!("@R {}", kept(&c.path)),
                format!("@H {}", kept(&c.hash)),
            ]
        });
        [
            vec![status.trim_end().to_owned()],
            self.holes
                .iter()
                .map(|h| format!("@? {}", kept(&h.to_string())))
                .collect(),
            self.unsaid
                .iter()
                .map(|u| unsaid(u).trim_end().to_owned())
                .collect(),
            courier.collect(),
            self.enqueues
                .iter()
                .map(|q| format!("@$ {}: {}", kept(&q.body), kept(&q.ask)))
                .collect(),
            self.verdicts
                .iter()
                .map(|v| format!("@V {}", one_line(v)))
                .collect(),
            self.findings
                .iter()
                .map(|f| format!("@F {}", one_line(&f.render())))
                .collect(),
            self.written
                .iter()
                .map(|w| format!("@W {}", one_line(w)))
                .collect(),
            self.tools
                .iter()
                .map(|t| format!("@T {}", one_line(t)))
                .collect(),
            self.forges
                .iter()
                .map(|f| format!("@FORGE {}", one_line(&f.to_string())))
                .collect(),
            self.other.iter().map(|o| tag::prose(o)).collect(),
        ]
    }

    /// The envelope lines without `@E`, in the order a cut must respect: what can never be cut
    /// comes first.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.sections().into_iter().flatten().collect()
    }

    /// Renders the report under a byte ceiling and closes it with an exact `@E`. A cap of 0
    /// means no ceiling. When the report does not fit, lines are dropped from the end of each
    /// list in this order — prose, `@T`, `@FORGE`, `@W`, `@F`, `@V` — and one hole names the cut,
    /// counting the lines earlier cuts dropped too (pointing at `dump` when one is given). `@S`,
    /// `@?`, `@U`, `@R`, `@H` and `@$` are never cut; a report made only of those may stay over
    /// its cap.
    #[must_use]
    pub fn emit(&self, cap: usize, dump: Option<&str>) -> String {
        let mut sections = self.sections();
        let whole = close(&sections);
        if cap == 0 || whole.len() <= cap || CUT_ORDER.iter().all(|&i| sections[i].is_empty()) {
            return whole;
        }
        // Earlier cuts fold into this one.
        let mut dropped = 0;
        let mut earlier_dump = None;
        let mut holes = Vec::with_capacity(self.holes.len());
        for (hole, line) in self.holes.iter().zip(sections[HOLES].drain(..)) {
            if let Hole::Cut {
                dropped: d, dump, ..
            } = hole
            {
                dropped += d;
                earlier_dump = earlier_dump.or(dump.clone());
            } else {
                holes.push(line);
            }
        }
        sections[HOLES] = holes;
        let dump = dump.map(str::to_owned).or(earlier_dump);
        let hole_line = |dropped| {
            format!(
                "@? {}",
                one_line(
                    &Hole::Cut {
                        cap,
                        dropped,
                        dump: dump.clone()
                    }
                    .to_string()
                )
            )
        };
        // Bytes of every line and the newlines between them, before the hole and `@E`.
        let mut body: usize = sections.iter().flatten().map(|l| l.len() + 1).sum();
        let fits =
            |body: usize, hole: &str| body + hole.len() + 1 + e_line(body + hole.len() + 1) <= cap;
        while let Some(&i) = CUT_ORDER.iter().find(|&&i| !sections[i].is_empty()) {
            let line = sections[i].pop().unwrap_or_default();
            body -= line.len() + 1;
            dropped += 1;
            if fits(body, &hole_line(dropped)) {
                break;
            }
        }
        sections[HOLES].push(hole_line(dropped));
        close(&sections)
    }

    /// The courier envelope of this report, whose full text — exactly `filed` — lies at `path`:
    /// the status, holes, unsaid and orders whole, `@R` and `@H` over `filed`, every path
    /// written, and the first eight verdicts and findings as a digest.
    #[must_use]
    pub fn envelope(&self, path: &str, filed: &str) -> Report {
        let mut env = Report {
            status: self.status.clone(),
            holes: self.holes.clone(),
            unsaid: self.unsaid.clone(),
            courier: Some(Courier {
                path: path.to_owned(),
                hash: Courier::hash_of(filed),
            }),
            enqueues: self.enqueues.clone(),
            written: self.written.clone(),
            ..Report::default()
        };
        env.verdicts = self.verdicts.iter().take(DIGEST_LINES).cloned().collect();
        let room = DIGEST_LINES.saturating_sub(env.verdicts.len());
        env.findings = self.findings.iter().take(room).cloned().collect();
        env
    }
}

impl std::fmt::Display for Report {
    /// The report with no ceiling.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.emit(0, None))
    }
}

/// The longest a field that a cap never cuts (`@S`, `@?`, `@U`, `@R`, `@H`, `@$`) is emitted, in
/// bytes; past it, the field ends in [`FIELD_CUT`].
pub const FIELD_BYTES: usize = 4 * crate::DEFAULT_CAP;

/// What ends a field bounded at [`FIELD_BYTES`].
const FIELD_CUT: &str = " …cut";

/// A never-cut field folded to one line and bounded at [`FIELD_BYTES`], the cut said.
fn kept(text: &str) -> Cow<'_, str> {
    let folded = one_line(text);
    if folded.len() <= FIELD_BYTES {
        return folded;
    }
    let mut end = FIELD_BYTES - FIELD_CUT.len();
    while !folded.is_char_boundary(end) {
        end -= 1;
    }
    Cow::Owned(format!("{}{FIELD_CUT}", &folded[..end]))
}

/// Every character some reader splits lines on: LF, CR, vertical tab, form feed, the
/// file, group and record separators, NEL, and the Unicode line and paragraph separators.
const BREAKS: [char; 10] = [
    '\n', '\r', '\u{0b}', '\u{0c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}', '\u{2029}',
];

/// Folds line breaks to spaces: what a field holds can never open a line of its own, whatever
/// splits the lines.
#[must_use]
pub fn one_line(text: &str) -> Cow<'_, str> {
    if text.contains(BREAKS) {
        Cow::Owned(text.replace("\r\n", " ").replace(BREAKS, " "))
    } else {
        Cow::Borrowed(text)
    }
}

fn push(list: &mut Vec<String>, item: String) -> bool {
    list.push(item);
    true
}

/// The length of `@E <n>` closing a body of `before` bytes (its newline counted in `before`):
/// `n` counts the whole answer, its own digits included — a fixed point.
fn e_line(before: usize) -> usize {
    let base = before + "@E ".len();
    let mut digits = 1;
    while (base + digits).to_string().len() != digits {
        digits += 1;
    }
    "@E ".len() + digits
}

/// Joins the sections' lines and appends the exact `@E <n>`.
fn close(sections: &[Vec<String>; SECTIONS]) -> String {
    let body = sections
        .iter()
        .flatten()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    let total = body.len() + 1 + e_line(body.len() + 1);
    format!("{body}\n@E {total}")
}

/// Scans any text for `@$ <body>: <ask>` order lines, so a relay a commission expects is
/// machine-readable before the body's report exists.
#[must_use]
pub fn enqueues_of(text: &str) -> Vec<Enqueue> {
    text.lines()
        .filter_map(|l| tag::split(l, false))
        .filter(|(t, _)| *t == "$")
        .filter_map(|(_, rest)| Enqueue::parse(&rest))
        .collect()
}
