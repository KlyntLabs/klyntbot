//! LLM-backed DayPlanningHandler implementation.

use std::sync::Arc;

use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use common::Result;
use jiff::Timestamp;
use providers::{ChatParams, DynProvider, Message, ResponseFormat};
use tracing::debug;

use feature_tasks::handlers::DayPlanningHandler;
use feature_tasks::types::{DayPlan, DeferredTask, PlanSlot, PlanSlotStatus, PlanningContext};

const DAY_PLAN_PROMPT: &str = include_str!("../templates/day_plan_prompt.md");

/// LLM response structure for day planning.
#[derive(serde::Deserialize)]
struct LlmDayPlanResponse {
    slots: Vec<LlmPlanSlot>,
    #[serde(default)]
    deferred: Vec<LlmDeferredTask>,
    total_work_mins: Option<i32>,
    utilization: Option<f64>,
    reasoning: String,
}

#[derive(serde::Deserialize)]
struct LlmPlanSlot {
    task_id: String,
    title: String,
    estimated_minutes: i32,
    start_time: Option<String>,
    energy_level: Option<String>,
}

#[derive(serde::Deserialize)]
struct LlmDeferredTask {
    task_id: String,
    title: String,
    reason: String,
}

/// LLM-powered day planning handler.
pub struct LlmDayPlanningHandler {
    provider: DynProvider,
    model: String,
    domain_bus: Option<Arc<DomainEventBus>>,
}

impl LlmDayPlanningHandler {
    pub fn new(
        provider: DynProvider,
        model: String,
        domain_bus: Option<Arc<DomainEventBus>>,
    ) -> Self {
        Self {
            provider,
            model,
            domain_bus,
        }
    }

    fn emit(&self, event: DomainEvent) {
        if let Some(bus) = &self.domain_bus {
            bus.publish(event);
        }
    }
}

#[async_trait]
impl DayPlanningHandler for LlmDayPlanningHandler {
    async fn plan_day(&self, context: &PlanningContext) -> Result<DayPlan> {
        let prompt = build_planning_prompt(context, &[]);
        let available_mins = compute_available_mins(&context.working_hours);
        let plan = call_llm_for_plan(&self.provider, &self.model, &prompt, available_mins).await?;

        self.emit(DomainEvent::DayPlanGenerated {
            task_count: plan.slots.len() as u32,
            total_estimated_mins: plan.total_work_mins as u32,
        });

        debug!(slots = plan.slots.len(), "Day plan generated");
        Ok(plan)
    }

    async fn replan(
        &self,
        context: &PlanningContext,
        current_plan: &DayPlan,
        reason: &str,
    ) -> Result<DayPlan> {
        let prompt = build_planning_prompt(context, &current_plan.locked_slots);
        let prompt_with_reason = format!(
            "{}\n\n## Replanning Reason\n{}\n\nKeep locked slots unchanged. Re-optimize remaining time.",
            prompt, reason
        );
        let available_mins = compute_available_mins(&context.working_hours);
        let mut plan = call_llm_for_plan(
            &self.provider,
            &self.model,
            &prompt_with_reason,
            available_mins,
        )
        .await?;

        // Preserve locked slots from the current plan
        plan.locked_slots = current_plan.locked_slots.clone();

        self.emit(DomainEvent::DayPlanGenerated {
            task_count: plan.slots.len() as u32,
            total_estimated_mins: plan.total_work_mins as u32,
        });

        debug!(
            slots = plan.slots.len(),
            "Day plan re-generated (reason: {})", reason
        );
        Ok(plan)
    }
}

/// Compute available working minutes from working hours (minus 30 min lunch).
fn compute_available_mins(wh: &feature_tasks::types::WorkingHours) -> i32 {
    // Convert times to minutes since midnight
    let end_mins = wh.end.hour() as i32 * 60 + wh.end.minute() as i32;
    let start_mins = wh.start.hour() as i32 * 60 + wh.start.minute() as i32;
    (end_mins - start_mins - 30).max(0)
}

async fn call_llm_for_plan(
    provider: &DynProvider,
    model: &str,
    prompt: &str,
    available_mins: i32,
) -> Result<DayPlan> {
    let params = ChatParams::new(model)
        .with_temperature(0.3)
        .with_max_tokens(4096)
        .with_response_format(ResponseFormat::JsonObject);

    let messages = vec![
        Message::system("You are a daily planning assistant. Return only valid JSON."),
        Message::user(prompt.to_string()),
    ];

    let response = provider.chat(&messages, None, &params).await?;
    let content = response.content.unwrap_or_default();
    let json_str = common::helpers::strip_llm_fences(&content);

    let parsed: LlmDayPlanResponse = serde_json::from_str(json_str).map_err(|e| {
        common::ToolError::ExecutionFailed(format!("Day plan JSON parse failed: {e}"))
    })?;

    let total_work_mins = parsed
        .total_work_mins
        .unwrap_or_else(|| parsed.slots.iter().map(|s| s.estimated_minutes).sum());

    Ok(DayPlan {
        slots: parsed
            .slots
            .into_iter()
            .map(|s| PlanSlot {
                task_id: s.task_id,
                title: s.title,
                estimated_minutes: s.estimated_minutes,
                energy_level: s.energy_level.as_deref().and_then(|e| e.parse().ok()),
                start_time: s.start_time,
                status: PlanSlotStatus::Pending,
            })
            .collect(),
        locked_slots: vec![],
        total_work_mins,
        available_mins,
        utilization: parsed.utilization.unwrap_or(0.0),
        reasoning: parsed.reasoning,
        deferred: parsed
            .deferred
            .into_iter()
            .map(|d| DeferredTask {
                task_id: d.task_id,
                title: d.title,
                reason: d.reason,
            })
            .collect(),
        generated_at: Timestamp::now(),
    })
}

fn build_planning_prompt(ctx: &PlanningContext, locked: &[PlanSlot]) -> String {
    let mut prompt = DAY_PLAN_PROMPT.to_string();

    // Format scored tasks
    let tasks_str = ctx
        .tasks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            format!(
                "{}. {} (ID: {}, est: {}min, energy: {}, P{})",
                i + 1,
                t.title,
                t.id,
                t.estimated_minutes.unwrap_or(30),
                t.energy_level
                    .as_ref()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "medium".to_string()),
                t.priority.unwrap_or(3),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    prompt = prompt.replace("{{scored_tasks}}", &tasks_str);
    prompt = prompt.replace(
        "{{work_start}}",
        &ctx.working_hours.start.strftime("%H:%M").to_string(),
    );
    prompt = prompt.replace(
        "{{work_end}}",
        &ctx.working_hours.end.strftime("%H:%M").to_string(),
    );
    prompt = prompt.replace(
        "{{lunch_start}}",
        &ctx.working_hours.lunch_start.strftime("%H:%M").to_string(),
    );

    // Calculate available minutes (work hours minus lunch)
    let work_mins = compute_available_mins(&ctx.working_hours);
    prompt = prompt.replace("{{available_mins}}", &work_mins.to_string());

    // Energy profile
    if let Some(ref profile) = ctx.energy_profile {
        prompt = prompt.replace("{{peak_hours}}", &format!("{:?}", profile.peak_hours));
        prompt = prompt.replace("{{low_hours}}", &format!("{:?}", profile.low_energy_hours));
        prompt = prompt.replace(
            "{{avg_focus_mins}}",
            &profile.avg_focus_duration_mins.unwrap_or(45).to_string(),
        );
    } else {
        prompt = prompt.replace("{{peak_hours}}", "[9, 10, 11]");
        prompt = prompt.replace("{{low_hours}}", "[14, 15]");
        prompt = prompt.replace("{{avg_focus_mins}}", "45");
    }

    // Calendar blocks
    let cal_str = if ctx.calendar_blocks.is_empty() {
        "(none)".to_string()
    } else {
        ctx.calendar_blocks
            .iter()
            .map(|b| {
                format!(
                    "- {} ({} - {}, {})",
                    b.title,
                    b.start.to_zoned(jiff::tz::TimeZone::UTC).strftime("%H:%M"),
                    b.end.to_zoned(jiff::tz::TimeZone::UTC).strftime("%H:%M"),
                    if b.is_busy { "busy" } else { "free" },
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    prompt = prompt.replace("{{calendar_blocks}}", &cal_str);

    // Locked slots
    let locked_str = if locked.is_empty() {
        "(none)".to_string()
    } else {
        locked
            .iter()
            .map(|s| {
                format!(
                    "- {} at {} ({}min)",
                    s.title,
                    s.start_time.as_deref().unwrap_or("?"),
                    s.estimated_minutes,
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    prompt = prompt.replace("{{locked_slots}}", &locked_str);

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use feature_tasks::types::{Task, WorkingHours};

    #[test]
    fn test_build_planning_prompt_includes_tasks() {
        let mut task = Task::default_instance();
        task.title = "Important task".to_string();
        task.estimated_minutes = Some(60);

        let ctx = PlanningContext {
            tasks: vec![task],
            working_hours: WorkingHours {
                start: jiff::civil::Time::new(9, 0, 0, 0).unwrap(),
                end: jiff::civil::Time::new(17, 0, 0, 0).unwrap(),
                lunch_start: jiff::civil::Time::new(12, 0, 0, 0).unwrap(),
            },
            calendar_blocks: vec![],
            energy_profile: None,
            max_tasks: None,
            locked_task_ids: vec![],
            target_date: None,
        };

        let prompt = build_planning_prompt(&ctx, &[]);
        assert!(prompt.contains("Important task"));
        assert!(prompt.contains("60min"));
        assert!(prompt.contains("09:00"));
        assert!(prompt.contains("17:00"));
    }

    #[test]
    fn test_calculate_available_mins_from_llm_response() {
        let plan = LlmDayPlanResponse {
            slots: vec![
                LlmPlanSlot {
                    task_id: "1".into(),
                    title: "A".into(),
                    estimated_minutes: 60,
                    start_time: None,
                    energy_level: None,
                },
                LlmPlanSlot {
                    task_id: "2".into(),
                    title: "B".into(),
                    estimated_minutes: 45,
                    start_time: None,
                    energy_level: None,
                },
            ],
            deferred: vec![],
            total_work_mins: Some(105),
            utilization: Some(0.5),
            reasoning: "test".into(),
        };
        // total_work_mins is provided, so it should use that
        assert_eq!(plan.total_work_mins.unwrap(), 105);
    }

    #[test]
    fn test_build_planning_prompt_with_locked_slots() {
        let ctx = PlanningContext {
            tasks: vec![],
            working_hours: WorkingHours::default(),
            calendar_blocks: vec![],
            energy_profile: None,
            max_tasks: None,
            locked_task_ids: vec![],
            target_date: None,
        };

        let locked = vec![PlanSlot {
            task_id: "locked-1".into(),
            title: "Stand-up".into(),
            estimated_minutes: 15,
            energy_level: None,
            start_time: Some("09:00".into()),
            status: PlanSlotStatus::Completed,
        }];

        let prompt = build_planning_prompt(&ctx, &locked);
        assert!(prompt.contains("Stand-up"));
        assert!(prompt.contains("09:00"));
        assert!(prompt.contains("15min"));
    }
}
