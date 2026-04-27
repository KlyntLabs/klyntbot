//! Work Context handlers: list, get detail, update, search, timeline, resume, stats, intelligence.

use std::collections::{BTreeMap, HashMap};

use cognitive::SemanticFactRepo;
use desktop_shared::commands::{
    ContextResumeResponse, ContextTimelineBlockResponse, DashboardIntelligenceResponse,
    DashboardNudge, InferenceConfigUpdate, InferenceStatsResponse, ResourceCluster, SessionBlock,
    WorkContextDetailResponse, WorkContextResponse, WorkContextSummary, WorkContextUpdateParams,
};
use desktop_shared::errors::ApiError;

use crate::errors::{map_activity_log_err as map_err, map_cognitive_err};
use crate::state::AppCore;

impl AppCore {
    // ── List / Get ───────────────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
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

    #[tracing::instrument(skip(self), err)]
    pub async fn get_work_context(
        &self,
        id: String,
    ) -> Result<Option<WorkContextResponse>, ApiError> {
        let ctx = activity_log::WorkContextRepo::get(&self.storage_pool, &id)
            .await
            .map_err(map_err)?;
        Ok(ctx.map(Into::into))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn get_work_context_detail(
        &self,
        id: String,
    ) -> Result<WorkContextDetailResponse, ApiError> {
        let ctx = activity_log::WorkContextRepo::get(&self.storage_pool, &id)
            .await
            .map_err(map_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("work context {id} not found")))?;

        let (resources, linked_task_ids, recent_events) = tokio::try_join!(
            async {
                activity_log::WorkResourceRepo::list_by_context(&self.storage_pool, &id)
                    .await
                    .map_err(map_err)
            },
            async {
                activity_log::ContextTaskRepo::list_tasks_for_context(&self.storage_pool, &id)
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
            linked_task_ids,
            recent_events: recent_events.into_iter().map(Into::into).collect(),
        })
    }

    // ── Mutations ────────────────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
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

        ctx.updated_at = jiff::Timestamp::now();
        activity_log::WorkContextRepo::update(&self.storage_pool, &ctx)
            .await
            .map_err(map_err)?;

        Ok(ctx.into())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn archive_work_context(&self, id: String) -> Result<WorkContextResponse, ApiError> {
        let mut ctx = activity_log::WorkContextRepo::get(&self.storage_pool, &id)
            .await
            .map_err(map_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("work context {id} not found")))?;

        ctx.status = activity_log::WorkContextStatus::Archived;
        ctx.updated_at = jiff::Timestamp::now();
        activity_log::WorkContextRepo::update(&self.storage_pool, &ctx)
            .await
            .map_err(map_err)?;

        Ok(ctx.into())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn merge_work_contexts(
        &self,
        keep_id: String,
        remove_id: String,
    ) -> Result<WorkContextResponse, ApiError> {
        activity_log::WorkContextRepo::merge(&self.storage_pool, &keep_id, &remove_id, "manual")
            .await
            .map_err(map_err)?;

        self.get_work_context(keep_id)
            .await?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "merged context not found"))
    }

    // ── Search ───────────────────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
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

    #[tracing::instrument(skip(self), err)]
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
            let offset = event.timestamp.duration_since(start).as_secs();
            let bucket_start = (offset / bucket_secs) * bucket_secs;
            let key = (bucket_start, event.work_context_id.clone());
            *buckets.entry(key).or_insert(0) += 1;
        }

        // Convert buckets to timeline blocks
        let mut blocks: Vec<ContextTimelineBlockResponse> = Vec::new();
        for ((bucket_offset, ctx_id), count) in &buckets {
            let block_start = start + jiff::SignedDuration::from_secs(*bucket_offset);
            let block_end = block_start + jiff::SignedDuration::from_secs(bucket_secs);

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
                start_time: block_start.to_string(),
                end_time: block_end.to_string(),
                event_count: *count,
                is_idle: ctx_id.is_none() && *count <= 1,
            });
        }

        // blocks are already ordered by (bucket_offset, ctx_id) from BTreeMap iteration
        Ok(blocks)
    }

    // ── Context Resume ───────────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
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

    // ── Inference Stats ───────────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
    pub async fn get_inference_stats(&self) -> Result<InferenceStatsResponse, ApiError> {
        let one_hour_ago = jiff::Timestamp::now() - jiff::SignedDuration::from_hours(1);
        let one_day_ago = jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24);

        let (
            active_count,
            archived_count,
            events_1h,
            events_24h,
            assigned_24h,
            avg_confidence,
            merges_24h,
        ) = tokio::try_join!(
            async {
                activity_log::WorkContextRepo::count_by_status(
                    &self.storage_pool,
                    activity_log::WorkContextStatus::Active,
                )
                .await
                .map_err(map_err)
            },
            async {
                activity_log::WorkContextRepo::count_by_status(
                    &self.storage_pool,
                    activity_log::WorkContextStatus::Archived,
                )
                .await
                .map_err(map_err)
            },
            async {
                activity_log::ActivityLogRepo::count_since(&self.storage_pool, one_hour_ago)
                    .await
                    .map_err(map_err)
            },
            async {
                activity_log::ActivityLogRepo::count_since(&self.storage_pool, one_day_ago)
                    .await
                    .map_err(map_err)
            },
            async {
                activity_log::ActivityLogRepo::count_assigned_since(&self.storage_pool, one_day_ago)
                    .await
                    .map_err(map_err)
            },
            async {
                activity_log::WorkContextRepo::avg_confidence_active(&self.storage_pool)
                    .await
                    .map_err(map_err)
            },
            async {
                activity_log::WorkContextRepo::count_merges_since(&self.storage_pool, one_day_ago)
                    .await
                    .map_err(map_err)
            },
        )?;

        let assignment_rate = if events_24h > 0 {
            assigned_24h as f64 / events_24h as f64
        } else {
            0.0
        };

        Ok(InferenceStatsResponse {
            active_context_count: active_count,
            archived_context_count: archived_count,
            events_last_hour: events_1h,
            events_last_24h: events_24h,
            assignment_rate,
            avg_confidence,
            merges_last_24h: merges_24h,
            last_run_at: activity_log::WorkContextRepo::get_last_inference_run(&self.storage_pool)
                .await
                .map_err(map_err)?,
        })
    }

    // ── Dashboard Intelligence ────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
    pub async fn get_dashboard_intelligence(
        &self,
        date: &str,
        tz_offset_mins: Option<i32>,
    ) -> Result<DashboardIntelligenceResponse, ApiError> {
        let (start, end) = crate::errors::parse_local_day_range(date, tz_offset_mins)?;
        let yesterday_start = start - jiff::SignedDuration::from_hours(24);

        // Fetch today's events, yesterday's events, active contexts, and semantic facts in parallel
        let fact_repo = SemanticFactRepo::new(self.repos.pool().clone());
        let (events, yesterday_events, contexts, work_facts) = tokio::try_join!(
            async {
                activity_log::ActivityLogRepo::query_range(
                    &self.storage_pool,
                    start,
                    end,
                    10_000,
                    0,
                )
                .await
                .map_err(map_err)
            },
            async {
                activity_log::ActivityLogRepo::query_range(
                    &self.storage_pool,
                    yesterday_start,
                    start,
                    10_000,
                    0,
                )
                .await
                .map_err(map_err)
            },
            async {
                activity_log::WorkContextRepo::list_active(&self.storage_pool)
                    .await
                    .map_err(map_err)
            },
            async {
                fact_repo
                    .list_active("productivity")
                    .await
                    .map_err(map_cognitive_err)
            },
        )?;

        let ctx_map: HashMap<String, &activity_log::WorkContext> =
            contexts.iter().map(|c| (c.id.clone(), c)).collect();

        // Find most recently active context (within 30 min)
        let thirty_mins_ago = jiff::Timestamp::now() - jiff::SignedDuration::from_mins(30);
        let active_context = contexts
            .iter()
            .filter(|c| c.last_active_at > thirty_mins_ago)
            .max_by_key(|c| c.last_active_at)
            .map(|c| WorkContextSummary {
                id: c.id.clone(),
                title: c.title.clone(),
                context_type: c.context_type.as_str().to_string(),
                color: c.color.clone(),
                duration_mins: c.total_duration_secs / 60,
                confidence: c.confidence,
            });

        // Build session summary by context type
        let mut type_durations: HashMap<String, (i64, i64, String)> = HashMap::new(); // (total_secs, count, color)
        let mut prev_context_id: Option<String> = None;
        let mut context_switches: i64 = 0;

        for event in &events {
            let dur = event.duration_secs.unwrap_or(0);
            let ctx_type = event
                .work_context_id
                .as_deref()
                .and_then(|id| ctx_map.get(id))
                .map(|c| {
                    (
                        c.context_type.as_str().to_string(),
                        c.color.clone().unwrap_or_else(|| "#6B7280".to_string()),
                    )
                })
                .unwrap_or_else(|| ("unassigned".to_string(), "#6B7280".to_string()));

            let entry = type_durations
                .entry(ctx_type.0.clone())
                .or_insert((0, 0, ctx_type.1));
            entry.0 += dur;
            entry.1 += 1;

            // Count context switches
            if event.work_context_id != prev_context_id && prev_context_id.is_some() {
                context_switches += 1;
            }
            prev_context_id = event.work_context_id.clone();
        }

        let session_summary: Vec<SessionBlock> = type_durations
            .into_iter()
            .filter(|(k, _)| k != "unassigned")
            .map(|(ctx_type, (secs, count, color))| SessionBlock {
                context_type: ctx_type,
                total_duration_mins: secs / 60,
                session_count: count,
                color,
            })
            .collect();

        let switch_quality = match context_switches {
            0..=3 => "low",
            4..=8 => "moderate",
            _ => "high",
        }
        .to_string();

        // Productivity score: ratio of time in known contexts vs total
        let total_event_secs: i64 = events.iter().filter_map(|e| e.duration_secs).sum();
        let assigned_secs: i64 = events
            .iter()
            .filter(|e| e.work_context_id.is_some())
            .filter_map(|e| e.duration_secs)
            .sum();
        let productivity_score = if total_event_secs > 0 {
            (assigned_secs as f64 / total_event_secs as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Focus recommendation based on active context
        let focus_recommendation = active_context.as_ref().map(|ctx| {
            if ctx.duration_mins > 120 {
                format!(
                    "You've been in {} mode for {}h — consider a short break",
                    ctx.context_type,
                    ctx.duration_mins / 60
                )
            } else {
                format!("Deep in {} — keep the momentum going!", ctx.context_type)
            }
        });

        // Coaching nudges — currently empty; will be populated when coaching
        // service exposes a pending_nudges() API.
        let nudges: Vec<DashboardNudge> = vec![];

        // Resource clusters from most-accessed resources today
        let resource_clusters = self.build_resource_clusters(&events, &ctx_map).await;

        Ok(DashboardIntelligenceResponse {
            active_context,
            focus_recommendation,
            session_summary,
            context_switches,
            switch_quality,
            productivity_score,
            score_trend: {
                let yday_total: i64 = yesterday_events
                    .iter()
                    .filter_map(|e| e.duration_secs)
                    .sum();
                let yday_assigned: i64 = yesterday_events
                    .iter()
                    .filter(|e| e.work_context_id.is_some())
                    .filter_map(|e| e.duration_secs)
                    .sum();
                let yday_score = if yday_total > 0 {
                    (yday_assigned as f64 / yday_total as f64).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                productivity_score - yday_score
            },
            patterns: work_facts
                .iter()
                .filter(|f| f.confidence >= 0.6)
                .take(5)
                .map(|f| format!("{} {} {}", f.subject, f.predicate, f.object))
                .collect(),
            nudges,
            resource_clusters,
        })
    }

    async fn build_resource_clusters(
        &self,
        events: &[activity_log::ActivityLogEntry],
        _ctx_map: &HashMap<String, &activity_log::WorkContext>,
    ) -> Vec<ResourceCluster> {
        // Group resources by context, find frequently co-accessed ones
        let mut resource_counts: HashMap<String, i64> = HashMap::new();
        for event in events {
            if let Some(ref name) = event.resource_name {
                *resource_counts.entry(name.clone()).or_insert(0) += 1;
            }
        }

        // Return top resources grouped by access count
        let mut sorted: Vec<(String, i64)> = resource_counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        if sorted.is_empty() {
            return vec![];
        }

        // Group top resources into a single cluster
        let top: Vec<(String, i64)> = sorted.into_iter().take(5).collect();
        let total_access: i64 = top.iter().map(|(_, c)| c).sum();
        vec![ResourceCluster {
            resources: top.into_iter().map(|(name, _)| name).collect(),
            access_count: total_access,
        }]
    }

    // ── Inference Config ──────────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
    pub async fn update_inference_config(
        &self,
        update: InferenceConfigUpdate,
    ) -> Result<(), ApiError> {
        let mut cfg = self.config.write().await;
        let wc = &mut cfg.work_context;

        if let Some(v) = update.assignment_threshold {
            wc.assignment_threshold = v;
        }
        if let Some(v) = update.merge_threshold {
            wc.merge_threshold = v;
        }
        if let Some(v) = update.semantic_weight {
            wc.semantic_weight = v;
        }
        if let Some(v) = update.temporal_weight {
            wc.temporal_weight = v;
        }
        if let Some(v) = update.resource_weight {
            wc.resource_weight = v;
        }
        if let Some(v) = update.inference_interval_mins {
            wc.inference_interval_mins = v;
        }
        if let Some(v) = update.max_dormancy_days {
            wc.max_dormancy_days = v;
        }
        if let Some(v) = update.max_active_contexts {
            wc.max_active_contexts = v;
        }

        config::save(&cfg)
            .await
            .map_err(crate::errors::map_config_save_err)?;
        Ok(())
    }
}
