//! Wraps ActionExecutor to probabilistically inject tool execution failures.

use chrono::{DateTime, Utc};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::actions::ActionExecutor;
use crate::persona::types::SimulatedToolAction;

/// Wraps an `ActionExecutor` and probabilistically injects failures.
pub struct ErrorInjector {
    executor: ActionExecutor,
    rng: std::sync::Mutex<StdRng>,
}

impl ErrorInjector {
    pub fn new(executor: ActionExecutor, seed: u64) -> Self {
        Self {
            executor,
            rng: std::sync::Mutex::new(StdRng::seed_from_u64(seed.wrapping_add(999))),
        }
    }

    /// Execute the action, or inject a failure based on error_injection_rate.
    /// Returns `(result, was_injected)`.
    pub async fn execute(
        &self,
        action: &SimulatedToolAction,
        simulated_now: DateTime<Utc>,
        error_injection_rate: f64,
    ) -> (common::Result<()>, bool) {
        if error_injection_rate > 0.0 {
            let (inject, error_type) = {
                let mut rng = self.rng.lock().unwrap();
                (
                    rng.random::<f64>() < error_injection_rate,
                    rng.random_range(0u8..4),
                )
            };
            if inject {
                let err = match error_type {
                    0 => common::KlyntbotError::Storage(
                        "table locked — concurrent write in progress".to_string(),
                    ),
                    1 => common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                        "entity not found: no matching note for query".to_string(),
                    )),
                    2 => common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                        "tool execution timed out after 30s".to_string(),
                    )),
                    _ => common::KlyntbotError::Tool(common::ToolError::InvalidParams(
                        "invalid argument: amount must be positive".to_string(),
                    )),
                };
                return (Err(err), true);
            }
        }

        (self.executor.execute(action, simulated_now).await, false)
    }
}
