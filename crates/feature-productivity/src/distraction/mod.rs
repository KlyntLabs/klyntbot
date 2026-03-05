pub mod classifier;
pub mod heuristics;
pub mod interceptor;

pub use classifier::{ContentClassification, DistractionClassifierHandler};
pub use heuristics::HeuristicVerdict;
pub use interceptor::{DistractionInterceptor, InterceptDecision};

#[cfg(test)]
mod tests;
