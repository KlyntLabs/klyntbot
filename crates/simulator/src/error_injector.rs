//! Probabilistic failure injection for simulation.

use chrono::{DateTime, Utc};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::actions::ActionExecutor;
use crate::persona::types::SimulatedToolAction;

/// Sample a random `KlyntbotError` to inject, or return `None` if the RNG
/// doesn't fire at the given rate. Used by both the heuristic-path
/// `ErrorInjector` and the agent-path `ErrorInjectingTool`.
pub fn sample_injected_error(rng: &mut StdRng, rate: f64) -> Option<common::KlyntbotError> {
    if rate <= 0.0 || rng.random::<f64>() >= rate {
        return None;
    }
    Some(match rng.random_range(0u8..4) {
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
    })
}

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
        let injected = {
            let mut rng = self.rng.lock().unwrap();
            sample_injected_error(&mut rng, error_injection_rate)
        };
        if let Some(err) = injected {
            return (Err(err), true);
        }
        (self.executor.execute(action, simulated_now).await, false)
    }
}
