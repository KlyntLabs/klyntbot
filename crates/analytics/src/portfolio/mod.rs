//! Portfolio analytics — drift detection, rebalancing, returns, correlations.

pub mod correlation;
pub mod drift;
pub mod returns;

pub use correlation::AssetCorrelationConfig;
pub use drift::{
    AssetAllocation, DriftResult, PortfolioAnalyzer, RebalanceResult, RebalanceStrategy,
    RebalanceSuggestion,
};
pub use returns::ReturnsResult;
