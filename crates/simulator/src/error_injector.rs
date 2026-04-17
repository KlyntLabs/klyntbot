//! Probabilistic failure injection for simulation.

use jiff::Timestamp;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use std::sync::atomic::{AtomicBool, Ordering};

use crate::actions::ActionExecutor;
use crate::persona::types::SimulatedToolAction;

// ── Cascade state ─────────────────────────────────────────────────────

/// Shared cascade state across all tools in one agent execution.
/// When a root error fires, downstream tools see elevated rates.
#[derive(Default)]
pub struct CascadeState {
    pub storage_failed: AtomicBool,
    pub timeout_fired: AtomicBool,
}

impl CascadeState {
    pub fn reset(&self) {
        self.storage_failed.store(false, Ordering::Relaxed);
        self.timeout_fired.store(false, Ordering::Relaxed);
    }
}

/// Compute effective error rate given cascade state.
pub fn cascade_adjusted_rate(
    base_rate: f64,
    tool_name: &str,
    state: &CascadeState,
    multiplier: f64,
) -> f64 {
    let mut rate = base_rate;

    // Storage failure elevates extraction and retrieval tools
    if state.storage_failed.load(Ordering::Relaxed) {
        let affected = ["memory", "notes", "tasks", "project", "finance"];
        if affected.iter().any(|t| tool_name.contains(t)) {
            rate = (rate * multiplier).min(0.8);
        }
    }

    // Timeout elevates all subsequent tools
    if state.timeout_fired.load(Ordering::Relaxed) {
        rate = (rate * (multiplier * 0.5)).min(0.5);
    }

    rate
}

/// Enhanced error sampling that updates cascade state.
pub fn sample_cascade_error(
    rng: &mut StdRng,
    rate: f64,
    state: &CascadeState,
) -> Option<common::KlyntbotError> {
    if rate <= 0.0 || rng.random::<f64>() >= rate {
        return None;
    }
    let err = match rng.random_range(0u8..4) {
        0 => {
            state.storage_failed.store(true, Ordering::Relaxed);
            common::KlyntbotError::Storage(
                "table locked — concurrent write in progress".to_string(),
            )
        }
        1 => common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
            "entity not found: no matching note for query".to_string(),
        )),
        2 => {
            state.timeout_fired.store(true, Ordering::Relaxed);
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                "tool execution timed out after 30s".to_string(),
            ))
        }
        _ => common::KlyntbotError::Tool(common::ToolError::InvalidParams(
            "invalid argument: amount must be positive".to_string(),
        )),
    };
    Some(err)
}

// ── Original flat injection ─────────────────────────────────────────��─

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
        simulated_now: Timestamp,
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
