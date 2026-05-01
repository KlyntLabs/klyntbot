//! Klynt execution policy — Starlark prefix-rule approval engine.
//! Adapted from codex-rs/execpolicy/.
//!
//! See `policy.rs` for the public `Policy` API.
//! See `parser.rs` for the Starlark grammar.
//! See `decision.rs` for the result type.

pub mod amend;
pub mod decision;
pub mod error;
pub mod executable_name;
pub mod parser;
pub mod policy;
pub mod rule;

pub use decision::Decision;
pub use error::{Error, Result};
pub use parser::parse_to_policy;
pub use policy::{Evaluation, Policy};
pub use rule::RuleMatch;
