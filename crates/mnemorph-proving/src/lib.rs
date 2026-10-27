//! The Dark Elf's proving: what makes a claim a finding.
//!
//! A Dark Elf's `@F` points at its record (`rec:`), a [`FindingRecord`] in the proving ground.
//! [`validate`] holds the record to the finding contract, rule by named rule, against the
//! [`World`] — the landing's tree — and gives a [`Valid`], the only type a count, a score or a
//! seal takes. [`accept`] is where the harness reads a Dark Elf's report: a finding whose record
//! breaks a rule rides back as an `@?` naming the rules it broke, never dropped silently.
//!
//! The rules adapt Cloudflare's `validate-findings.cjs` (MIT; `THIRD-PARTY-NOTICES.md`). The
//! contract is in the book, `docs/02-races.md`.

mod accept;
mod finding;
mod world;

pub use accept::{Accepted, accept};
pub use finding::{
    CLAIM_BYTES, FIELD_BYTES, FindingRecord, Impact, LIST_ITEMS, Likelihood, RECORD_BYTES, Rule,
    Severity, Step, StepKind, TEXT_BYTES, TRACE_STEPS, Valid, Verdict, Witness, WitnessKind,
    record_path, validate,
};
pub use world::{READ_BYTES, Tree, World};

#[cfg(test)]
mod tests;
