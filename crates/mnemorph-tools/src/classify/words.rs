//! Words as the shell will see them: braces expanded, quotes removed, `~` and `$HOME` resolved.
//! Anything else the shell would expand at run time leaves the word unknown.

use std::path::Path;

use brush_parser::ParserOptions;
use brush_parser::word::{
    self, BraceExpressionMember, BraceExpressionOrText, Parameter, ParameterExpr, TildeExpr,
    WordPiece, WordPieceWithSource,
};

/// How many words one brace expression may expand into before the classifier refuses to guess.
const MAX_BRACE_WORDS: usize = 256;

/// One word after expansion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Lit {
    /// The literal text, or `None` when part of it is known only at run time (a variable, a
    /// command substitution).
    pub(crate) text: Option<String>,
    /// The command substitutions inside the word, each classified on its own.
    pub(crate) subs: Vec<String>,
}

impl Lit {
    /// A word whose value the classifier cannot know (what `xargs` feeds its command).
    pub(crate) fn unknown() -> Self {
        Self {
            text: None,
            subs: Vec::new(),
        }
    }
}

/// Expand one raw word into the words the shell will pass.
pub(crate) fn expand(raw: &str, home: &Path) -> Result<Vec<Lit>, String> {
    let options = ParserOptions::default();
    // Only a quote-led word is a literal pattern; any other is brace-expanded first.
    let alternatives = if raw.starts_with(['\'', '"']) {
        vec![raw.to_owned()]
    } else {
        match word::parse_brace_expansions(raw, &options) {
            Ok(Some(parts)) => {
                let mut out = vec![String::new()];
                braces(&parts, &mut out)?;
                out
            }
            Ok(None) | Err(_) => vec![raw.to_owned()],
        }
    };
    alternatives
        .iter()
        .map(|alt| {
            let pieces = word::parse(alt, &options)
                .map_err(|e| format!("a word does not parse as shell: {e}"))?;
            let mut lit = Lit {
                text: Some(String::new()),
                subs: Vec::new(),
            };
            fold(&pieces, home, &mut lit);
            Ok(lit)
        })
        .collect()
}

/// Expand brace parts onto every prefix in `out`, as the shell does (left to right, cartesian).
fn braces(parts: &[BraceExpressionOrText], out: &mut Vec<String>) -> Result<(), String> {
    for part in parts {
        match part {
            BraceExpressionOrText::Text(s) => out.iter_mut().for_each(|o| o.push_str(s)),
            BraceExpressionOrText::Expr(members) => {
                let mut options: Vec<String> = Vec::new();
                for m in members {
                    match m {
                        BraceExpressionMember::Child(children) => {
                            let mut sub = vec![String::new()];
                            braces(children, &mut sub)?;
                            options.extend(sub);
                        }
                        BraceExpressionMember::NumberSequence {
                            start,
                            end,
                            increment,
                        } => {
                            sequence(*start, *end, *increment, |n| options.push(n.to_string()))?;
                        }
                        BraceExpressionMember::CharSequence {
                            start,
                            end,
                            increment,
                        } => {
                            sequence(
                                i64::from(u32::from(*start)),
                                i64::from(u32::from(*end)),
                                *increment,
                                |n| {
                                    if let Some(c) = u32::try_from(n).ok().and_then(char::from_u32)
                                    {
                                        options.push(c.to_string());
                                    }
                                },
                            )?;
                        }
                    }
                }
                let mut next = Vec::with_capacity(out.len() * options.len());
                for prefix in out.iter() {
                    for o in &options {
                        next.push(format!("{prefix}{o}"));
                    }
                }
                if next.len() > MAX_BRACE_WORDS {
                    return Err("a brace expression expands into too many words to read".into());
                }
                *out = next;
            }
        }
    }
    Ok(())
}

fn sequence(start: i64, end: i64, increment: i64, mut each: impl FnMut(i64)) -> Result<(), String> {
    let step = increment.unsigned_abs().max(1);
    let count = start.abs_diff(end) / step + 1;
    if count > MAX_BRACE_WORDS as u64 {
        return Err("a brace sequence expands into too many words to read".into());
    }
    let mut n = start;
    for _ in 0..count {
        each(n);
        n = if start <= end {
            n.saturating_add_unsigned(step)
        } else {
            n.saturating_sub_unsigned(step)
        };
    }
    Ok(())
}

fn fold(pieces: &[WordPieceWithSource], home: &Path, lit: &mut Lit) {
    for p in pieces {
        match &p.piece {
            WordPiece::Text(s) | WordPiece::SingleQuotedText(s) | WordPiece::AnsiCQuotedText(s) => {
                push(lit, s);
            }
            WordPiece::EscapeSequence(s) => push(lit, s.strip_prefix('\\').unwrap_or(s)),
            WordPiece::DoubleQuotedSequence(inner)
            | WordPiece::GettextDoubleQuotedSequence(inner) => {
                fold(inner, home, lit);
            }
            WordPiece::TildeExpansion(TildeExpr::Home) => push(lit, &home.to_string_lossy()),
            WordPiece::ParameterExpansion(ParameterExpr::Parameter {
                parameter: Parameter::Named(name),
                indirect: false,
            }) if name == "HOME" => push(lit, &home.to_string_lossy()),
            WordPiece::CommandSubstitution(src) | WordPiece::BackquotedCommandSubstitution(src) => {
                lit.subs.push(src.clone());
                lit.text = None;
            }
            _ => lit.text = None,
        }
    }
}

fn push(lit: &mut Lit, s: &str) {
    if let Some(t) = &mut lit.text {
        t.push_str(s);
    }
}
