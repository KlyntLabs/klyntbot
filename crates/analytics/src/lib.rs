//! Analytics crate — pure computation for financial analysis.
//!
//! No storage dependency, no async, no side effects.
//! All inputs are pre-fetched data; all outputs are computation results.
//! Every function that uses randomness accepts a seed for reproducibility.

pub mod input_types;
pub mod monte_carlo;
pub mod types;

pub mod fire;

pub mod portfolio;
pub mod spending;

// Explicit re-exports to avoid name collisions (e.g., AllocationTarget)
pub use input_types::{
    AllocationTarget, Holding, InvestmentCashFlow, PriceSeries, RecurringFrequency, SpendingRecord,
    SpendingType,
};
pub use monte_carlo::{
    AssetClass, InflationModel, MonteCarloEngine, ReturnModel, SimulationConfig, SimulationResult,
    TerminalStats, WithdrawalStrategy, WorstSequence,
};
pub use types::{
    Anomaly, AnomalyDirection, AnomalySeverity, CorrelationMatrix, PercentileBands, TimeSeries,
    TrendDirection,
};
