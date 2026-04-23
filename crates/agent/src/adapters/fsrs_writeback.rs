//! FSRS write-back bridge — persists promoted trial params to the cognitive repo.
//!
//! Isolated from the nightly cycle so it can be tested independently.

use common::TrialParams;

/// Write `fsrs_desired_retention` from promoted trial params into the cognitive
/// `fsrs_parameters` table.
///
/// No-op when the param is `None`.
pub async fn apply_fsrs_writeback(
    repo: &cognitive::FsrsParamsRepo,
    params: &TrialParams,
) -> common::Result<()> {
    if let Some(retention) = params.fsrs_desired_retention {
        repo.update_desired_retention(retention).await?;
    }
    Ok(())
}
