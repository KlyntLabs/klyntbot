//! TaskComplexitySignals — Evaluate task complexity for execution decisions.
//!
//! Scores task complexity based on dependencies, subtasks, estimated duration,
//! and priority. Ported from feature-todo's `task_complexity.rs`, updated
//! to use `TaskRepo` instead of `ActionRepo`.

use storage::TaskRepo;

/// Complexity signals derived from a task's structure and metadata.
#[derive(Debug, Clone)]
pub struct TaskComplexitySignals {
    pub has_dependencies: bool,
    pub subtask_count: u16,
    pub dependency_depth: u8,
    pub estimated_minutes: Option<i32>,
    pub priority: i16,
}

impl TaskComplexitySignals {
    /// Compute an integer complexity score (0-6).
    pub fn complexity_score(&self) -> u8 {
        let mut score: u8 = 0;
        if self.has_dependencies {
            score += 2;
        }
        if self.subtask_count >= 3 {
            score += 1;
        }
        if self.dependency_depth >= 2 {
            score += 1;
        }
        if self.estimated_minutes.unwrap_or(0) >= 60 {
            score += 1;
        }
        if self.priority <= 2 {
            score += 1;
        }
        score
    }

    /// Returns `true` if the complexity score meets or exceeds the threshold.
    pub fn exceeds_complexity_threshold(&self, threshold: u8) -> bool {
        self.complexity_score() >= threshold
    }
}

/// Query a task's structural complexity from the database.
pub async fn evaluate_task_complexity(repo: &TaskRepo, task_id: &str) -> TaskComplexitySignals {
    // Count direct subtasks
    let subtask_count = repo
        .count_children(task_id)
        .await
        .map(|(total, _)| total)
        .unwrap_or(0) as u16;

    // Check if this task has any blockers
    let blockers = repo.get_blockers(task_id).await.unwrap_or_default();
    let has_dependencies = !blockers.is_empty();

    // Dependency depth: count how many levels of blockers exist
    let dependency_depth = if has_dependencies {
        compute_dependency_depth(repo, task_id, 0, 5).await
    } else {
        0
    };

    // Load the task itself for estimated_minutes and priority
    let (estimated_minutes, priority) = match repo.get(task_id).await {
        Ok(Some(row)) => (row.estimated_minutes, row.priority.unwrap_or(3)),
        _ => (None, 3),
    };

    TaskComplexitySignals {
        has_dependencies,
        subtask_count,
        dependency_depth,
        estimated_minutes,
        priority,
    }
}

/// Recursively compute dependency depth with a max-depth guard.
async fn compute_dependency_depth(repo: &TaskRepo, task_id: &str, current: u8, max: u8) -> u8 {
    if current >= max {
        return current;
    }
    let blockers = repo.get_blockers(task_id).await.unwrap_or_default();
    if blockers.is_empty() {
        return current;
    }
    let mut deepest = current + 1;
    for blocker in &blockers {
        let d = Box::pin(compute_dependency_depth(
            repo,
            &blocker.id,
            current + 1,
            max,
        ))
        .await;
        if d > deepest {
            deepest = d;
        }
    }
    deepest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_task_below_threshold() {
        let signals = TaskComplexitySignals {
            has_dependencies: false,
            subtask_count: 0,
            dependency_depth: 0,
            estimated_minutes: Some(15),
            priority: 3,
        };
        assert!(!signals.exceeds_complexity_threshold(3));
        assert_eq!(signals.complexity_score(), 0);
    }

    #[test]
    fn complex_task_exceeds_complexity_threshold() {
        let signals = TaskComplexitySignals {
            has_dependencies: true,
            subtask_count: 5,
            dependency_depth: 3,
            estimated_minutes: Some(120),
            priority: 1,
        };
        assert!(signals.exceeds_complexity_threshold(3));
        // has_deps(2) + subtasks>=3(1) + depth>=2(1) + est>=60(1) + priority<=2(1) = 6
        assert_eq!(signals.complexity_score(), 6);
    }

    #[test]
    fn dependencies_only_scores_2() {
        let signals = TaskComplexitySignals {
            has_dependencies: true,
            subtask_count: 0,
            dependency_depth: 0,
            estimated_minutes: None,
            priority: 4,
        };
        assert_eq!(signals.complexity_score(), 2);
    }

    #[test]
    fn high_priority_long_task() {
        let signals = TaskComplexitySignals {
            has_dependencies: false,
            subtask_count: 1,
            dependency_depth: 0,
            estimated_minutes: Some(120),
            priority: 1,
        };
        // est>=60(1) + priority<=2(1) = 2
        assert_eq!(signals.complexity_score(), 2);
        assert!(!signals.exceeds_complexity_threshold(3));
    }

    #[test]
    fn threshold_boundary() {
        let signals = TaskComplexitySignals {
            has_dependencies: true,
            subtask_count: 3,
            dependency_depth: 1,
            estimated_minutes: None,
            priority: 4,
        };
        // has_deps(2) + subtasks>=3(1) = 3
        assert_eq!(signals.complexity_score(), 3);
        assert!(signals.exceeds_complexity_threshold(3));
        assert!(!signals.exceeds_complexity_threshold(4));
    }
}
