use std::collections::HashMap;

/// Priority levels for context window content, ordered from highest to lowest.
/// Higher priority content is allocated space first in the waterfall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    SystemIdentity = 0,
    ActiveTask = 1,
    ToolDefinitions = 2,
    RecentHistory = 3,
    RetrievedMemory = 4,
    CompressedHistory = 5,
    BootstrapPersona = 6,
    Skills = 7,
}

/// Configuration for the token budget.
pub struct BudgetConfig {
    pub total_context_window: usize,
    pub response_reserve_pct: f32,
}

impl BudgetConfig {
    /// Standard config: 15% reserved for response generation.
    pub fn standard(window: usize) -> Self {
        Self {
            total_context_window: window,
            response_reserve_pct: 0.15,
        }
    }

    /// Tokens reserved for the model's response.
    pub fn response_reserve(&self) -> usize {
        (self.total_context_window as f32 * self.response_reserve_pct) as usize
    }

    /// Tokens available for input content.
    pub fn available_input(&self) -> usize {
        self.total_context_window - self.response_reserve()
    }
}

/// Tracks token allocations across priority levels.
pub struct BudgetAllocator {
    config: BudgetConfig,
    allocations: HashMap<Priority, usize>,
}

/// Summary of budget usage.
#[derive(Clone)]
pub struct BudgetReport {
    pub total_window: usize,
    pub total_allocated: usize,
    pub remaining: usize,
    pub per_priority: Vec<(Priority, usize)>,
}

impl BudgetAllocator {
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            config,
            allocations: HashMap::new(),
        }
    }

    /// Total tokens allocated across all priorities.
    pub fn total_allocated(&self) -> usize {
        self.allocations.values().sum()
    }

    /// Tokens still available for allocation.
    pub fn remaining(&self) -> usize {
        self.config
            .available_input()
            .saturating_sub(self.total_allocated())
    }

    /// Allocate up to `tokens` for the given priority, capped at remaining budget.
    pub fn allocate(&mut self, priority: Priority, tokens: usize) {
        let can_add = self.remaining().min(tokens);
        if can_add > 0 {
            *self.allocations.entry(priority).or_insert(0) += can_add;
        }
    }

    /// Try to allocate `tokens`, returning how many were actually allocated.
    pub fn try_allocate(&mut self, priority: Priority, tokens: usize) -> usize {
        let can_add = self.remaining().min(tokens);
        if can_add > 0 {
            *self.allocations.entry(priority).or_insert(0) += can_add;
        }
        can_add
    }

    /// Get the current allocation for a specific priority.
    pub fn get(&self, priority: Priority) -> usize {
        self.allocations.get(&priority).copied().unwrap_or(0)
    }

    /// Generate a budget usage report.
    pub fn report(&self) -> BudgetReport {
        let total_allocated = self.total_allocated();
        let mut per_priority: Vec<(Priority, usize)> =
            self.allocations.iter().map(|(k, v)| (*k, *v)).collect();
        per_priority.sort_by_key(|(p, _)| *p as u8);
        BudgetReport {
            total_window: self.config.total_context_window,
            total_allocated,
            remaining: self
                .config
                .available_input()
                .saturating_sub(total_allocated),
            per_priority,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_config_standard() {
        let config = BudgetConfig::standard(128_000);
        assert_eq!(config.response_reserve(), 19_200); // 15% of 128000
        assert_eq!(config.available_input(), 108_800); // 85% of 128000
    }

    #[test]
    fn test_allocate_within_budget() {
        let mut allocator = BudgetAllocator::new(BudgetConfig::standard(128_000));
        allocator.allocate(Priority::SystemIdentity, 500);
        allocator.allocate(Priority::ActiveTask, 2000);
        allocator.allocate(Priority::ToolDefinitions, 3000);
        allocator.allocate(Priority::RecentHistory, 10000);
        assert!(allocator.remaining() > 0);
        assert_eq!(allocator.total_allocated(), 15500);
    }

    #[test]
    fn test_try_allocate_respects_budget_cap() {
        // 1000 token window, 15% reserve = 850 available
        let mut allocator = BudgetAllocator::new(BudgetConfig::standard(1000));
        allocator.allocate(Priority::SystemIdentity, 500);
        allocator.allocate(Priority::ActiveTask, 300);
        // 50 tokens left
        let allocated = allocator.try_allocate(Priority::Skills, 200);
        assert!(allocated <= 50, "Should not exceed remaining budget");
        assert!(allocated < 200, "Should be less than requested");
    }

    #[test]
    fn test_budget_report() {
        let mut allocator = BudgetAllocator::new(BudgetConfig::standard(128_000));
        allocator.allocate(Priority::SystemIdentity, 500);
        let report = allocator.report();
        assert_eq!(report.total_window, 128_000);
        assert_eq!(report.total_allocated, 500);
        assert!(report.remaining > 0);
        assert!(!report.per_priority.is_empty());
    }

    #[test]
    fn test_allocate_capped_at_available() {
        let mut allocator = BudgetAllocator::new(BudgetConfig::standard(1000));
        allocator.allocate(Priority::SystemIdentity, 99999);
        // Should be capped at available_input (850)
        assert_eq!(allocator.total_allocated(), 850);
    }
}
