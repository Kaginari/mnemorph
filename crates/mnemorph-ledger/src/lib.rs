//! The usage ledger: weighted tokens, prices, budgets and envelopes, caps, folds; and the value
//! ledger's Dark Elf folds, findings per speciality ([`fold_findings`]).
//!
//! Budget flows down like context; spend flows up like memory. Every model call writes one
//! [`UsageRow`] to an append-only journal; envelopes carve the session's budget down the chain;
//! caps and folds read the journal back. The book's page is `docs/06-consumption.md`.

mod caps;
mod clock;
mod envelope;
mod findings;
mod fold;
mod journal;
mod prices;
mod row;
mod usage;

pub use caps::{
    CapReading, Caps, UNSEEN_FLOOR_WT, UNSEEN_RESERVE_PCT, check_caps, unseen_against, unseen_wt,
};
pub use clock::{Day, Timestamp};
pub use envelope::{
    BudgetShort, CHALLENGE_RESERVE_PCT, Envelope, Item, PARENT_RESERVE_PCT, SettleError, Slice,
    TopUpError,
};
pub use findings::{
    Ascension, CANNOT_VERIFY_BAR, FindingRow, SpecialityFold, Standing, WINDOW, fold_findings,
};
pub use fold::{Key, Totals, fold};
pub use journal::{Journal, ReadBack};
pub use prices::{DEFAULT_OUTPUT_RATIO, Price, Prices, PricesError};
pub use row::{DecisionRow, EnvelopeReading, OutcomeRow, UsageRow};
pub use usage::{Usage, estimate, occupancy, spend, wt};
