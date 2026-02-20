//! Skill context sources — summary and always-loaded skill content.

use std::sync::Arc;

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};

use crate::skills::SkillManager;

/// Provides the skills summary section (skill names, descriptions, triggers).
pub struct SkillSummarySource {
    skills: Arc<SkillManager>,
}

impl SkillSummarySource {
    pub fn new(skills: Arc<SkillManager>) -> Self {
        Self { skills }
    }
}

#[async_trait]
impl ContextSource for SkillSummarySource {
    fn name(&self) -> &str {
        "skills_summary"
    }

    fn priority(&self) -> u8 {
        40
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let summary = self.skills.generate_summary();
        Some(format!("# Available Skills\n\n{}", summary))
    }
}

/// Provides the full content of always-loaded skills.
pub struct SkillContentSource {
    skills: Arc<SkillManager>,
}

impl SkillContentSource {
    pub fn new(skills: Arc<SkillManager>) -> Self {
        Self { skills }
    }
}

#[async_trait]
impl ContextSource for SkillContentSource {
    fn name(&self) -> &str {
        "skills_content"
    }

    fn priority(&self) -> u8 {
        30
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let always_loaded = self.skills.get_always_loaded();
        if always_loaded.is_empty() {
            return None;
        }

        let mut sections = Vec::with_capacity(always_loaded.len());
        for skill in always_loaded {
            if let Some(content) = &skill.content {
                sections.push(format!("# Skill: {}\n\n{}", skill.name, content));
            }
        }

        if sections.is_empty() {
            None
        } else {
            Some(sections.join("\n\n---\n\n"))
        }
    }
}
