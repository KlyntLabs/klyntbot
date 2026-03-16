//! FIRE (Financial Independence, Retire Early) calculator suite.

pub mod sensitivity;
pub mod sequence_risk;
pub mod variants;
pub mod withdrawal;

pub use sensitivity::{SensitivityConfig, SensitivityPoint, SensitivityResult};
pub use sequence_risk::{HistoricalBacktestParams, HistoricalBacktestResult};
pub use variants::*;
pub use withdrawal::{WithdrawalParams, WithdrawalResult};
