//! Work Context handlers: list, get detail, update, search, timeline, resume.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use desktop_shared::commands::{
    ContextResumeResponse, ContextTimelineBlockResponse, WorkContextDetailResponse,
    WorkContextResponse, WorkContextUpdateParams,
};
use desktop_shared::errors::ApiError;

use crate::errors::map_activity_log_err as map_err;
use crate::state::AppCore;

impl AppCore {
    // ── List / Get ───────────────────────────────────────────────────

    pub async fn list_work_contexts(
        &self,
        status: Option<String>,
    ) -> Result<Vec<WorkContextResponse>, ApiError> {
        let contexts = if let Some(s) = status {
            let parsed = activity_log::WorkContextStatus::parse(&s)
                .ok_or_else(|| ApiError::new("VALIDATION", format!("invalid status: {s}")))?;
            activity_log::WorkContextRepo::list_by_status(&self.storage_pool, parsed, 200)
                .await
                .map_err(map_err)?
        } else {
            activity_log::WorkContextRepo::list_active(&self.storage_pool)
                .await
                .map_err(map_err)?
        };
        Ok(contexts.into_iter().map(Into::into).collect())
    }

    pub async fn get_work_context(
        &self,
        id: String,
    ) -> Result<Option<WorkContextResponse>, ApiError> {
        let ctx = activity_log::WorkContextRepo::get(&self.storage_pool, &id)
            .await
            .map_err(map_err)?;
        Ok(ctx.map(Into::into))
    }

    pub async fn get_work_context_detail(
        &self,
        id: String,
    ) -> Result<WorkContextDetailResponse, ApiError> {
        let ctx = activity_log::WorkContextRepo::get(&self.storage_pool, &id)
            .await
            .map_err(map_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("work context {id} not found")))?;

        let (resources, linked_action_ids, recent_events) = tokio::try_join!(
            async {
                activity_log::WorkResourceRepo::list_by_context(&self.storage_pool, &id)
                    .await
                    .map_err(map_err)
            },
            async {
                activity_log::ContextActionRepo::list_actions_for_context(&self.storage_pool, &id)
                    .await
                    .map_err(map_err)
            },
            async {
                activity_log::ActivityLogRepo::query_by_context(&self.storage_pool, &id, 50)
                    .await
                    .map_err(map_err)
            },
        )?;

        Ok(WorkContextDetailResponse {
            context: ctx.into(),
            resources: resources.into_iter().map(Into::into).collect(),
            linked_action_ids,
            recent_events: recent_events.into_iter().map(Into::into).collect(),
        })
    }

    // ── Mutations ────────────────────────────────────────────────────

    pub async fn update_work_context(
        &self,
        params: WorkContextUpdateParams,
    ) -> Result<WorkContextResponse, ApiError> {
        let mut ctx = activity_log::WorkContextRepo::get(&self.storage_pool, &params.id)
            .await
            .map_err(map_err)?
            .ok_or_else(|| {
                ApiError::new("NOT_FOUND", format!("work context {} not found", params.id))
            })?;

        if let Some(title) = params.title {
            ctx.title = title;
        }
        if let Some(color) = params.color {
            ctx.color = Some(color);
        }
        if let Some(status) = params.status {
            ctx.status = activity_log::WorkContextStatus::parse(&status)
                .ok_or_else(|| ApiError::new("VALIDATION", format!("invalid status: {status}")))?;
        }
        if let Some(project_id) = params.linked_project_id {
            ctx.linked_project_id = if project_id.is_empty() {
                None
            } else {
                Some(project_id)
            };
        }

        ctx.updated_at = Utc::now();
        activity_log::WorkContextRepo::update(&self.storage_pool, &ctx)
            .await
            .map_err(map_err)?;

        Ok(ctx.into())
    }

    pub async fn archive_work_context(&self, id: String) -> Result<WorkContextResponse, ApiError> {
        let mut ctx = activity_log::WorkContextRepo::get(&self.storage_pool, &id)
            .await
            .map_err(map_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("work context {id} not found")))?;

        ctx.status = activity_log::WorkContextStatus::Archived;
        ctx.updated_at = Utc::now();
        activity_log::WorkContextRepo::update(&self.storage_pool, &ctx)
            .await
            .map_err(map_err)?;

        Ok(ctx.into())
    }

    pub async fn merge_work_contexts(
        &self,
        keep_id: String,
        remove_id: String,
    ) -> Result<WorkContextResponse, ApiError> {
        activity_log::WorkContextRepo::merge(&self.storage_pool, &keep_id, &remove_id)
            .await
            .map_err(map_err)?;

        self.get_work_context(keep_id)
            .await?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "merged context not found"))
    }

    // ── Search ───────────────────────────────────────────────────────

    pub async fn search_work_contexts(
        &self,
        query: String,
    ) -> Result<Vec<WorkContextResponse>, ApiError> {
        let results = activity_log::WorkContextRepo::search_by_title(&self.storage_pool, &query)
            .await
            .map_err(map_err)?;
        Ok(results.into_iter().map(Into::into).collect())
    }

    // ── Context Timeline ─────────────────────────────────────────────

    pub async fn get_context_timeline(
        &self,
        date: String,
        tz_offset_mins: Option<i32>,
    ) -> Result<Vec<ContextTimelineBlockResponse>, ApiError> {
        let (start, end) = crate::errors::parse_local_day_range(&date, tz_offset_mins)?;

        // Fetch all events for the day
        let events =
            activity_log::ActivityLogRepo::query_range(&self.storage_pool, start, end, 10_000, 0)
                .await
                .map_err(map_err)?;

        if events.is_empty() {
            return Ok(vec![]);
        }

        // Load all active+paused contexts for color/title lookup
        let contexts = activity_log::WorkContextRepo::list_active(&self.storage_pool)
            .await
            .map_err(map_err)?;
        let ctx_map: std::collections::HashMap<String, &activity_log::WorkContext> =
            contexts.iter().map(|c| (c.id.clone(), c)).collect();

        // Group events into 5-minute buckets keyed by (bucket_start, context_id)
        let bucket_secs: i64 = 300;
        let mut buckets: BTreeMap<(i64, Option<String>), i64> = BTreeMap::new();

        for event in &events {
            let offset = (event.timestamp - start).num_seconds();
            let bucket_start = (offset / bucket_secs) * bucket_secs;
            let key = (bucket_start, event.work_context_id.clone());
            *buckets.entry(key).or_insert(0) += 1;
        }

        // Convert buckets to timeline blocks
        let mut blocks: Vec<ContextTimelineBlockResponse> = Vec::new();
        for ((bucket_offset, ctx_id), count) in &buckets {
            let block_start = start + Duration::seconds(*bucket_offset);
            let block_end = block_start + Duration::seconds(bucket_secs);

            let (title, color, ctx_type) = ctx_id
                .as_deref()
                .and_then(|cid| ctx_map.get(cid))
                .map(|c| {
                    (
                        Some(c.title.clone()),
                        c.color.clone(),
                        Some(c.context_type.as_str().to_string()),
                    )
                })
                .unwrap_or((None, None, None));

            blocks.push(ContextTimelineBlockResponse {
                context_id: ctx_id.clone(),
                context_title: title,
                context_color: color,
                context_type: ctx_type,
                start_time: block_start.to_rfc3339(),
                end_time: block_end.to_rfc3339(),
                event_count: *count,
                is_idle: ctx_id.is_none() && *count <= 1,
            });
        }

        // blocks are already ordered by (bucket_offset, ctx_id) from BTreeMap iteration
        Ok(blocks)
    }

    // ── Context Resume ───────────────────────────────────────────────

    pub async fn get_context_resume_data(
        &self,
        context_id: String,
    ) -> Result<ContextResumeResponse, ApiError> {
        let ctx = activity_log::WorkContextRepo::get(&self.storage_pool, &context_id)
            .await
            .map_err(map_err)?
            .ok_or_else(|| {
                ApiError::new("NOT_FOUND", format!("work context {context_id} not found"))
            })?;

        let resources =
            activity_log::WorkResourceRepo::list_by_context(&self.storage_pool, &context_id)
                .await
                .map_err(map_err)?;

        let resource_names: Vec<String> = resources
            .iter()
            .take(10)
            .map(|r| r.resource_name.clone())
            .collect();

        // Template-based summary (no LLM needed)
        let hours = ctx.total_duration_secs as f64 / 3600.0;
        let top_resources = resource_names[..resource_names.len().min(3)].join(", ");

        let summary = if top_resources.is_empty() {
            format!(
                "{:.1}h on \"{}\" ({} events)",
                hours, ctx.title, ctx.event_count
            )
        } else {
            format!(
                "{:.1}h on \"{}\", primarily {}. {} events total.",
                hours, ctx.title, top_resources, ctx.event_count
            )
        };

        let suggested_prompt = format!("Continue working on {}", ctx.title);

        Ok(ContextResumeResponse {
            context_id: ctx.id,
            context_title: ctx.title,
            summary,
            suggested_prompt,
            recent_resources: resource_names,
        })
    }
}
