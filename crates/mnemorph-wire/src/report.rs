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
/// id, a location or a witness. The parser drops one at each end.
const GUARD: &str = "—";

/// One `@F #<id> <file:line> <claim> witness:<path>` finding. Every part but the claim is
/// optional; the id lets a later fix, contest or seal point at it, the witness is what makes a
/// Dark Elf's finding count.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Finding {
    /// `#id`, without the `#`. An empty id is no id.
    pub id: Option<String>,
    /// `file:line` or `file:line-line`.
    pub at: Option<String>,
    /// The claim; its whitespace is normalised to single spaces.
    pub claim: String,
    /// `witness:<path>`: a failing proof or a scanner output.
    pub witness: Option<String>,
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
        if words.last() == Some(&GUARD) {
            words.pop();
        } else if let Some(last) = words.last()
            && let Some(path) = last.strip_prefix("witness:")
            && !path.is_empty()
        {
            f.witness = Some(path.to_owned());
            words.pop();
        }
        f.claim = words.join(" ");
        f
    }

    /// The finding's content address: ten hex digits of FNV-1a over its location, claim and
    /// witness, so a relayed finding keeps the same address at every hop. Use it as the id of a
    /// finding that has none.
    #[must_use]
    pub fn address(&self) -> String {
        let claim = self.claim.split_whitespace().collect::<Vec<_>>().join(" ");
        let key = format!(
            "{}|{claim}|{}",
            self.at.as_deref().unwrap_or(""),
            self.witness.as_deref().unwrap_or("")
        );
        format!("{:016x}", fnv::fnv64a(key.as_bytes()))[..10].to_owned()
    }

    fn render(&self) -> String {
        let id = self.id.as_deref().filter(|i| !i.is_empty());
        let claim: Vec<&str> = self.claim.split_whitespace().collect();
        let first = claim.first().copied();
        let last = claim.last().copied();
        // A claim word that the parser would take for something else is fenced with GUARD.
        let lead = first.is_some_and(|w| {
            w == GUARD
                || (self.at.is_none()
                    && (is_location(w) || (id.is_none() && w.len() > 1 && w.starts_with('#'))))
        });
        let trail = self.witness.is_none()
            && last.is_some_and(|w| {
                w == GUARD || w.strip_prefix("witness:").is_some_and(|p| !p.is_empty())
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
        if let Some(w) = &self.witness {
            let _ = write!(out, " witness:{w}");
        }
        out.trim().to_owned()
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
        let mut seen = false;
        let (mut path, mut hash) = (None, None);
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let Some((t, rest)) = tag::split(line, raw) else {
                r.other.push(line.to_owned());
                continue;
            };
            let kept = match t {
                "S" => {
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
                "E" => {
                    r.bytes = rest.split_whitespace().next().and_then(|n| n.parse().ok());
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

    /// The envelope lines by section, without `@E`. Every field is folded to one line, so no
    /// text can open a line of its own.
    fn sections(&self) -> [Vec<String>; SECTIONS] {
        let status = format!("@S {}", one_line(&self.status).trim());
        let unsaid = |u: &Unsaid| match u.kind {
            Some(k) => format!("@U {} {}", k.word(), one_line(&u.text)),
            None => format!("@U {}", one_line(&u.text)),
        };
        let courier = self.courier.iter().flat_map(|c| {
            [
                format!("@R {}", one_line(&c.path)),
                format!("@H {}", one_line(&c.hash)),
            ]
        });
        [
            vec![status.trim_end().to_owned()],
            self.holes
                .iter()
                .map(|h| format!("@? {}", one_line(&h.to_string())))
                .collect(),
            self.unsaid
                .iter()
                .map(|u| unsaid(u).trim_end().to_owned())
                .collect(),
            courier.collect(),
            self.enqueues
                .iter()
                .map(|q| format!("@$ {}: {}", one_line(&q.body), one_line(&q.ask)))
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
            self.other
                .iter()
                .map(|o| one_line(o).into_owned())
                .collect(),
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

/// Folds line breaks to spaces: what a field holds can never open a line of its own.
pub(crate) fn one_line(text: &str) -> Cow<'_, str> {
    if text.contains(['\n', '\r']) {
        Cow::Owned(text.replace("\r\n", " ").replace(['\n', '\r'], " "))
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
