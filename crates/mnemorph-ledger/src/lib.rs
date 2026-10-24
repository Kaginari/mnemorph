//! The usage ledger: weighted tokens, prices, budgets and envelopes, caps, folds.
//!
//! Budget flows down like context; spend flows up like memory. Every model call writes one
//! [`UsageRow`] to an append-only journal; envelopes carve the session's budget down the chain;
//! caps and folds read the journal back. The book's page is `docs/06-consumption.md`.

mod caps;
mod clock;
mod envelope;
mod fold;
mod journal;
mod prices;
mod row;
mod usage;

pub use caps::{CapReading, Caps, UNSEEN_FLOOR_WT, check_caps, unseen_wt};
pub use clock::{Day, Timestamp};
pub use envelope::{
    BudgetShort, CHALLENGE_RESERVE_PCT, Envelope, Item, PARENT_RESERVE_PCT, SettleError, Slice,
    TopUpError,
};
pub use fold::{Key, Totals, fold};
pub use journal::{Journal, ReadBack};
pub use prices::{DEFAULT_OUTPUT_RATIO, Price, Prices, PricesError};
pub use row::{DecisionRow, EnvelopeReading, OutcomeRow, UsageRow};
pub use usage::{Usage, estimate, occupancy, spend, wt};
