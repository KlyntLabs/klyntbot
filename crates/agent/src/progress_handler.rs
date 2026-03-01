//! ProgressHandlerImpl — cascades action completion through KR → Objective progress.

use async_trait::async_trait;
use common::Result;
use tools_core::ProgressHandler;
use storage::{ActionRepo, KeyResultRepo, ObjectiveRepo};
use tracing::debug;

pub struct ProgressHandlerImpl {
    kr_repo: KeyResultRepo,
    objective_repo: ObjectiveRepo,
    action_repo: ActionRepo,
}

impl ProgressHandlerImpl {
    pub fn new(
        kr_repo: KeyResultRepo,
        objective_repo: ObjectiveRepo,
        action_repo: ActionRepo,
    ) -> Self {
        Self {
            kr_repo,
            objective_repo,
            action_repo,
        }
    }
}

#[async_trait]
impl ProgressHandler for ProgressHandlerImpl {
    async fn recalculate_kr_progress(&self, key_result_id: &str) -> Result<()> {
        // Fetch KR and action counts concurrently (both are independent reads).
        let (kr, counts) = tokio::try_join!(
            self.kr_repo.get(key_result_id),
            self.action_repo.count_by_kr(key_result_id),
        )?;

        if let Some(kr) = kr {
            if kr.tracking_mode == "action" {
                let (total, completed) = counts;
                let progress = if total > 0 {
                    completed as f64 / total as f64 * 100.0
                } else {
                    0.0
                };
                self.kr_repo
                    .update_progress(key_result_id, progress)
                    .await?;
                debug!(
                    key_result_id,
                    completed, total, progress, "KR progress updated from action completion"
                );
            }
            // Recalculate parent objective progress (avg of all KR progresses)
            let obj_progress = self
                .objective_repo
                .recalculate_progress(&kr.objective_id)
                .await?;
            debug!(
                objective_id = kr.objective_id,
                obj_progress, "Objective progress recalculated"
            );
        }
        Ok(())
    }
}
