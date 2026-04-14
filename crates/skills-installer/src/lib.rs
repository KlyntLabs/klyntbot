//! skills-installer: transactional install/upgrade/uninstall on top of SkillStore + EntityStore.

pub mod installer;
pub mod plan;
pub mod seeder;
pub mod uninstall;

pub use installer::Installer;
pub use plan::{InstallPlan, TemplatePreview, UpgradePlan};
pub use seeder::seed_bundled;
pub use uninstall::UninstallMode;

#[cfg(test)]
mod tests_install;
