//! The report: what a body sends back.

use std::fmt::Write as _;

use crate::{fnv, forge::Forge, tag};

/// A report longer than this travels as a courier envelope: the full text in a file, the
/// reader receives [`Report::envelope`].
pub const COURIER_BYTES: usize = 600;

/// How many lines of findings and verdicts a courier envelope carries inline.
const DIGEST_LINES: usize = 8;

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
    fn word(self) -> &'static str {
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

/// One `@F #<id> <file:line> <claim> witness:<path>` finding. Every part but the claim is
/// optional; the id lets a later fix, contest or seal point at it, the witness is what makes a
/// Dark Elf's finding count.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Finding {
    /// `#id`, without the `#`.
    pub id: Option<String>,
    /// `file:line` or `file:line-line`.
    pub at: Option<String>,
    /// The claim.
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
        if let Some(last) = words.last()
            && let Some(path) = last.strip_prefix("witness:")
            && !path.is_empty()
        {
            f.witness = Some(path.to_owned());
            words.pop();
        }
        // The raw pipe form separates location and claim with a dash.
        if f.at.is_some() && words.first() == Some(&"—") {
            words.remove(0);
        }
        f.claim = words.join(" ");
        f
    }

    fn render(&self) -> String {
        let mut out = String::new();
        if let Some(id) = &self.id {
            let _ = write!(out, "#{id} ");
        }
        if let Some(at) = &self.at {
            let _ = write!(out, "{at} ");
        }
        out.push_str(&self.claim);
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

/// What a body sends back.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// `@S` — the status: a word (`DONE`, `PASS`, `FAIL`, …) and anything after it.
    pub status: String,
    /// `@?` — holes: what could not be settled, never guessed.
    pub holes: Vec<String>,
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
    /// `@E` — the byte length as read; set by [`Report::parse`] only.
    pub bytes: Option<usize>,
}

impl Report {
    /// Reads a report. It tolerates prose around the envelope (kept in [`Report::other`]) and
    /// the raw pipe form (`S|PASS`, `F|path:ln|claim`). `None` when no `@S` opened an answer:
    /// such a text is not on the wire.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut r = Report::default();
        let mut seen = false;
        let (mut path, mut hash) = (None, None);
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let Some((t, rest)) = tag::split(line, true) else {
                r.other.push(line.to_owned());
                continue;
            };
            let kept = match t {
                "S" => {
                    r.status = rest;
                    seen = true;
                    true
                }
                "?" => push(&mut r.holes, rest),
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

    /// The first word of the status, the word every fold reads (`DONE`, `FAIL`, …).
    #[must_use]
    pub fn word(&self) -> &str {
        self.status.split_whitespace().next().unwrap_or("")
    }

    /// The envelope lines without `@E`, in the order a cut must respect: what can never be cut
    /// comes first.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut out = vec![format!("@S {}", self.status.trim()).trim_end().to_owned()];
        out.extend(self.holes.iter().map(|h| format!("@? {h}")));
        out.extend(self.unsaid.iter().map(|u| match u.kind {
            Some(k) => format!("@U {} {}", k.word(), u.text).trim_end().to_owned(),
            None => format!("@U {}", u.text),
        }));
        if let Some(c) = &self.courier {
            out.push(format!("@R {}", c.path));
            out.push(format!("@H {}", c.hash));
        }
        out.extend(
            self.enqueues
                .iter()
                .map(|q| format!("@$ {}: {}", q.body, q.ask)),
        );
        out.extend(self.verdicts.iter().map(|v| format!("@V {v}")));
        out.extend(self.findings.iter().map(|f| format!("@F {}", f.render())));
        out.extend(self.written.iter().map(|w| format!("@W {w}")));
        out.extend(self.tools.iter().map(|t| format!("@T {t}")));
        out.extend(self.forges.iter().map(|f| format!("@FORGE {f}")));
        out.extend(self.other.iter().cloned());
        out
    }

    /// Renders the report under a byte ceiling and closes it with an exact `@E`. A cap of 0
    /// means no ceiling. When the report does not fit, lines are dropped from the end of each
    /// list in this order — prose, `@T`, `@FORGE`, `@W`, `@F`, `@V` — and a hole names the cut
    /// (pointing at `dump` when one is given). `@S`, `@?`, `@U`, `@R`, `@H` and `@$` are never
    /// cut; a report made only of those may stay over its cap.
    #[must_use]
    pub fn emit(&self, cap: usize, dump: Option<&str>) -> String {
        let whole = close(&self.lines());
        if cap == 0 || whole.len() <= cap {
            return whole;
        }
        let mut keep = self.clone();
        let mut dropped = 0usize;
        loop {
            let cut = keep.other.pop().is_some()
                || keep.tools.pop().is_some()
                || keep.forges.pop().is_some()
                || keep.written.pop().is_some()
                || keep.findings.pop().is_some()
                || keep.verdicts.pop().is_some();
            if !cut && dropped == 0 {
                // Only lines that are never cut: the report stays whole, over its cap.
                return whole;
            }
            if cut {
                dropped += 1;
            }
            let mut trial = keep.clone();
            trial.holes.push(cut_hole(cap, dropped, dump));
            let out = close(&trial.lines());
            if !cut || out.len() <= cap {
                return out;
            }
        }
    }

    /// The courier envelope of this report, whose full text lies at `path`: the status, holes,
    /// unsaid and orders whole, `@R` and `@H`, every path written, and the first eight verdicts
    /// and findings as a digest.
    #[must_use]
    pub fn envelope(&self, path: &str) -> Report {
        let full = self.emit(0, None);
        let mut env = Report {
            status: self.status.clone(),
            holes: self.holes.clone(),
            unsaid: self.unsaid.clone(),
            courier: Some(Courier {
                path: path.to_owned(),
                hash: format!("fnv64:{:016x}", fnv::fnv64a(full.as_bytes())),
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

fn push(list: &mut Vec<String>, item: String) -> bool {
    list.push(item);
    true
}

fn cut_hole(cap: usize, dropped: usize, dump: Option<&str>) -> String {
    let plural = if dropped == 1 { "" } else { "s" };
    let mut hole = format!("@CAP {cap}: {dropped} line{plural} cut");
    if let Some(dump) = dump {
        let _ = write!(hole, " — full report at {dump}");
    }
    hole
}

/// Joins the lines and appends `@E <n>`, where `n` is the byte length of the whole answer
/// including the `@E` line itself — a fixed point on the number of digits.
fn close(lines: &[String]) -> String {
    let body = lines.join("\n");
    let base = body.len() + "\n@E ".len();
    let mut digits = 1;
    let total = loop {
        let n = base + digits;
        if n.to_string().len() == digits {
            break n;
        }
        digits += 1;
    };
    format!("{body}\n@E {total}")
}

/// Scans any text for `@$ <body>: <ask>` order lines, so a relay a commission expects is
/// machine-readable before the body's report exists.
#[must_use]
pub fn enqueues_of(text: &str) -> Vec<Enqueue> {
    text.lines()
        .filter_map(|l| tag::split(l, true))
        .filter(|(t, _)| *t == "$")
        .filter_map(|(_, rest)| Enqueue::parse(&rest))
        .collect()
}
