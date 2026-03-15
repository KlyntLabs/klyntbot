pub mod classifier;
pub mod heuristics;
pub mod interceptor;
pub mod monitor;

pub use classifier::{ContentClassification, DistractionClassifierHandler};
pub use heuristics::HeuristicVerdict;
pub use interceptor::{DistractionInterceptor, InterceptDecision};
pub use monitor::{DistractionAlert, DistractionMonitor};

#[cfg(test)]
mod tests;
