//! skills-adapter: LLM transformation of prompt-only skills into Klynt-native ones.

pub mod adapter;
pub mod prompt;

pub use adapter::{AdaptedSkill, Adapter, AdapterOutput, AdapterProvider};
