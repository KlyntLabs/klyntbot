//! FSRS-5 spaced-repetition concern, extracted from the `cognitive` god-crate.
//!
//! Card scheduling, review logs, the FSRS optimizer, and per-domain retention.
//! A pure leaf: depends only on `storage`/`common` — no other cognitive concern.
//! `cognitive` re-exports these modules so existing `cognitive::*` paths keep
//! working; schema/migrations live in `cognitive-schema`.

pub mod deck_preference;
pub mod flashcard;
pub mod fsrs5;
pub mod fsrs_optimizer;
pub mod fsrs_params;
pub mod retention_history;
pub mod review_session;
pub mod review_stats;
