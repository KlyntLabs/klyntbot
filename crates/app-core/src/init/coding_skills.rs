use crate::AppCore;
use common::Result;

#[tracing::instrument(skip(core), err)]
pub async fn init_coding_skills(core: &AppCore) -> Result<()> {
    core.coding_skills_reload().await?;
    let count = core.coding_skills_list().await?.len();
    tracing::info!(skills_indexed = count, "coding skill loader initialized");
    Ok(())
}
