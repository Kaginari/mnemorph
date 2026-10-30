//! The usage ledger: weighted tokens, prices, budgets and envelopes, caps, folds; and the value
//! ledger's Dark Elf folds, findings per speciality ([`fold_findings`]); the uses of Oni
//! envelopes ([`Uses`]), counted under a lock at the moment of use; and an evolution judged after
//! the freeze, on its post-swap window against doing nothing ([`evolution`]).
//!
//! Budget flows down like context; spend flows up like memory. Every model call writes one
//! [`UsageRow`] to an append-only journal; envelopes carve the session's budget down the chain;
//! caps and folds read the journal back; a journal may chain its rows by
//! hash ([`chain`]), so a silent edit is found. The book's page is `docs/06-consumption.md`.

mod build;
mod caps;
pub mod chain;
mod clock;
mod envelope;
pub mod evolution;
mod findings;
mod fold;
mod journal;
mod prices;
mod row;
mod usage;
mod uses;

pub use build::{Build, UNKNOWN};
pub use caps::{
    CapReading, Caps, UNSEEN_FLOOR_WT, UNSEEN_RESERVE_PCT, check_caps, unseen_against, unseen_wt,
};
pub use chain::{Break, Head, Report};
pub use clock::{Day, Timestamp};
pub use envelope::{
    BudgetShort, CHALLENGE_RESERVE_PCT, Envelope, Item, PARENT_RESERVE_PCT, SettleError, Shortfall,
    Slice, TopUpError,
};
pub use findings::{
    Ascension, CANNOT_VERIFY_BAR, FindingRow, Proposed, SpecialityFold, Standing, WINDOW,
    fold_findings, fold_findings_by_build,
};
pub use fold::{Key, Totals, fold};
pub use journal::{Journal, ReadBack, parse as parse_rows};
pub use prices::{DEFAULT_OUTPUT_RATIO, Price, Prices, PricesError};
pub use row::{DecisionRow, EnvelopeReading, OutcomeRow, UsageRow};
pub use usage::{Usage, estimate, occupancy, spend, wt};
pub use uses::{Held, UseRow, Uses};
