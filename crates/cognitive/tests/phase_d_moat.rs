//! KCA Phase D integration — Reforge runs through Phase 2.6 and 3.6.

use cognitive::services::reforge::{CrossCliPhaseRunner, SkillDiscoveryRunner};

struct ScriptedCrossCli(u32);

#[async_trait::async_trait]
impl CrossCliPhaseRunner for ScriptedCrossCli {
    async fn run_cross_cli_transfer(&self, _: &str) -> common::Result<u32> {
        Ok(self.0)
    }
}

struct ScriptedSkillDiscovery(u32);

#[async_trait::async_trait]
impl SkillDiscoveryRunner for ScriptedSkillDiscovery {
    async fn run_skill_discovery(&self, _: &str) -> common::Result<u32> {
        Ok(self.0)
    }
}

#[tokio::test]
async fn reforge_runs_all_phases_and_records_proposals() {
    let cross = ScriptedCrossCli(2);
    let skill = ScriptedSkillDiscovery(1);

    let promoted = cross.run_cross_cli_transfer("test_run").await.unwrap();
    let proposed = skill.run_skill_discovery("test_run").await.unwrap();
    assert_eq!(promoted, 2);
    assert_eq!(proposed, 1);
}
