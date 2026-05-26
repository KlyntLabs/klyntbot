//! Knowledge-graph concern, extracted from the `cognitive` god-crate.
//!
//! Entity/edge storage, community detection (Louvain), personalized-PageRank
//! retrieval, per-turn graph linking, graph enrichment, and community
//! intelligence. Sits below `cognitive-memory` in the DAG: memory's `scoring`
//! and `retrieval` call into `louvain`/`ppr_retrieval`/`graph_retrieval`, so the
//! graph concern owns `co_activation` too (both memory and graph use it).
//!
//! A leaf: depends only on `context_engine`, `cognitive-schema`, and primitives —
//! never on memory/reforge/mirror. `cognitive` re-exports these modules so the
//! existing `cognitive::repos::*` / `cognitive::services::*` paths keep resolving.

// Repos (formerly cognitive::repos::*)
pub mod book_tree;
pub mod co_activation;
pub mod community;
pub mod entity;
pub mod gt_link;
pub mod markdown_parser;

// Services (formerly cognitive::services::*)
pub mod community_intelligence;
pub mod community_membership_online;
pub mod graph_enrichment;
pub mod graph_linker;
pub mod graph_linker_types;
pub mod graph_retrieval;
pub mod louvain;

// Flat re-exports kept for the few internal call sites that used the crate-root
// repo paths (mirrors what `cognitive::repos` re-exported).
pub use co_activation::CoActivationRepo;
pub use entity::NewEntity;
