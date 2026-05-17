//! Klynt Cognitive Architecture benchmark crate.
//!
//! Real, Letta-comparable evaluations only. Synthetic fixture runners were
//! removed 2026-05-01 — they were eval-tuned and gave false-green signals
//! while real LoCoMo was at ~30%.

pub mod cost;
pub mod latency;
pub mod locomo_real;
pub mod trace;
