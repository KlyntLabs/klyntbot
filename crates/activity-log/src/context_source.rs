use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use jiff::Timestamp;
use storage::StoragePool;

use crate::context_resource_repo::ContextResourceRepo;
use crate::types::WorkContextStatus;
use crate::work_context_repo::WorkContextRepo;

pub struct WorkContextSource {
    pool: StoragePool,
}

impl WorkContextSource {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ContextSource for WorkContextSource {
    fn name(&self) -> &str {
        "work_context"
    }

    fn priority(&self) -> u8 {
        55
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let contexts = WorkContextRepo::list_by_status(&self.pool, WorkContextStatus::Active, 3)
            .await
            .ok()?;
        if contexts.is_empty() {
            return None;
        }

        let mut output = String::from("[Current Work Contexts]\n");
        for (i, ctx) in contexts.iter().enumerate() {
            let age_secs = (Timestamp::now().as_second() - ctx.last_active_at.as_second()).max(0);
            let age_mins = age_secs / 60;
            let age_str = if age_mins < 60 {
                format!("{age_mins}min ago")
            } else {
                format!("{}h ago", age_mins / 60)
            };
            let duration_str = format!("{:.1}h today", ctx.total_duration_secs as f64 / 3600.0);

            let label = if i == 0 { "Active" } else { "Recent" };
            output.push_str(&format!(
                "{label}: \"{}\" ({}, {duration_str})\n",
                ctx.title, age_str
            ));

            if let Ok(resources) = ContextResourceRepo::list_for_context(&self.pool, &ctx.id).await
            {
                if !resources.is_empty() {
                    let names: Vec<&str> = resources
                        .iter()
                        .take(3)
                        .map(|(r, _)| r.resource_name.as_str())
                        .collect();
                    output.push_str(&format!("  Key resources: {}\n", names.join(", ")));
                }
            }
        }

        Some(output)
    }

    fn estimated_tokens(&self) -> usize {
        300
    }
}
