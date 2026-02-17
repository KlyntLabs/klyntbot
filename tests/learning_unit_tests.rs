//! Inline unit test skeletons for the Learning System (Phase 3).
//!
//! These tests are organized by the module they belong to.
//! IMPLEMENTERS: Copy each `mod tests { ... }` block into the end of the
//! corresponding source file (e.g., types.rs, outcome_store.rs, etc.).
//!
//! Run: `cargo test -p agent -- learning`

// ═══════════════════════════════════════════════════════════════════════════
// FILE: crates/agent/src/learning/types.rs
// Copy the block below to the bottom of types.rs
// ═══════════════════════════════════════════════════════════════════════════
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use chrono::Utc;
//
//     fn make_outcome_record(tool_name: &str, success: bool, confidence: f32) -> OutcomeRecord {
//         OutcomeRecord {
//             id: uuid::Uuid::new_v4().to_string(),
//             session_key: format!("telegram:{:x}", 0xdeadbeef_u32),
//             tool_name: tool_name.to_string(),
//             success,
//             error_category: if success { None } else { Some("validation".to_string()) },
//             duration_ms: 100,
//             confidence_score: Some(confidence),
//             confidence_dimensions: None,
//             execution_mode: ExecutionMode::Chat,
//             created_at: Utc::now(),
//         }
//     }
//
//     // ─── Serde round-trip tests ───────────────────────────────────────────
//
//     #[test]
//     fn test_outcome_record_serde_round_trip() {
//         let record = make_outcome_record("todo", true, 0.85);
//         let json = serde_json::to_string(&record).expect("serialize");
//         let back: OutcomeRecord = serde_json::from_str(&json).expect("deserialize");
//         assert_eq!(record.id, back.id);
//         assert_eq!(record.tool_name, back.tool_name);
//         assert_eq!(record.success, back.success);
//         assert!((record.confidence_score.unwrap() - back.confidence_score.unwrap()).abs() < f32::EPSILON);
//     }
//
//     #[test]
//     fn test_execution_mode_plan_step_serde() {
//         let mode = ExecutionMode::PlanStep {
//             plan_id: "plan-abc".to_string(),
//             step_index: 2,
//         };
//         let json = serde_json::to_string(&mode).unwrap();
//         let back: ExecutionMode = serde_json::from_str(&json).unwrap();
//         assert!(matches!(back, ExecutionMode::PlanStep { step_index: 2, .. }));
//     }
//
//     #[test]
//     fn test_adaptive_threshold_state_serde_round_trip() {
//         let state = AdaptiveThresholdState {
//             current_threshold: 0.72,
//             last_analysis: None,
//             threshold_history: vec![ThresholdChange {
//                 from: 0.7,
//                 to: 0.72,
//                 reason: "analysis".to_string(),
//                 timestamp: Utc::now(),
//             }],
//             updated_at: Utc::now(),
//         };
//         let json = serde_json::to_string(&state).unwrap();
//         let back: AdaptiveThresholdState = serde_json::from_str(&json).unwrap();
//         assert!((back.current_threshold - 0.72).abs() < f32::EPSILON);
//         assert_eq!(back.threshold_history.len(), 1);
//     }
//
//     #[test]
//     fn test_analysis_result_serde_round_trip() {
//         // TODO: construct an AnalysisResult with all fields populated
//         // Assert: serde round-trip is lossless
//         todo!("implement after AnalysisResult struct is finalized")
//     }
//
//     #[test]
//     fn test_enrichment_stats_default_is_zero() {
//         let stats = EnrichmentStats::default();
//         assert_eq!(stats.total_suggestions, 0);
//         assert_eq!(stats.accepted_count, 0);
//         assert!((stats.acceptance_rate - 0.0).abs() < f32::EPSILON);
//         assert!(stats.per_field.is_empty());
//     }
//
//     #[test]
//     fn test_confidence_band_serde() {
//         let band = ConfidenceBand {
//             lower: 0.7,
//             upper: 0.85,
//             total: 10,
//             successes: 8,
//             success_rate: 0.8,
//         };
//         let json = serde_json::to_string(&band).unwrap();
//         let back: ConfidenceBand = serde_json::from_str(&json).unwrap();
//         assert!((back.lower - 0.7).abs() < f32::EPSILON);
//         assert_eq!(back.total, 10);
//     }
//
//     // ─── Field presence tests (privacy-by-omission enforcement) ──────────
//
//     #[test]
//     fn test_outcome_record_has_no_args_field() {
//         // Ensure OutcomeRecord JSON never contains "args" or "user_message"
//         let record = make_outcome_record("todo", true, 0.8);
//         let json = serde_json::to_string(&record).unwrap();
//         assert!(!json.contains("\"args\""), "args should not be serialized");
//         assert!(!json.contains("\"arguments\""), "arguments should not be serialized");
//         assert!(!json.contains("\"user_message\""), "user_message should not be serialized");
//     }
// }

// ═══════════════════════════════════════════════════════════════════════════
// FILE: crates/agent/src/learning/outcome_store.rs
// Copy the block below to the bottom of outcome_store.rs
// ═══════════════════════════════════════════════════════════════════════════
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use chrono::Utc;
//     use tempfile::TempDir;
//
//     async fn create_test_store() -> (OutcomeStore, TempDir) {
//         let temp_dir = TempDir::new().unwrap();
//         let path = temp_dir.path().join("outcomes.jsonl");
//         let store = OutcomeStore::new(path);
//         (store, temp_dir)
//     }
//
//     fn make_record(id: &str, tool: &str, success: bool) -> OutcomeRecord {
//         use crate::learning::types::*;
//         OutcomeRecord {
//             id: id.to_string(),
//             session_key: "telegram:abc123".to_string(),
//             tool_name: tool.to_string(),
//             success,
//             error_category: None,
//             duration_ms: 50,
//             confidence_score: Some(0.8),
//             confidence_dimensions: None,
//             execution_mode: ExecutionMode::Chat,
//             created_at: Utc::now(),
//         }
//     }
//
//     fn make_feedback(task_id: &str, field: &str, accepted: bool) -> EnrichmentFeedbackEntry {
//         EnrichmentFeedbackEntry {
//             task_id: task_id.to_string(),
//             field: field.to_string(),
//             suggested_value: "1".to_string(),
//             actual_value: if accepted { None } else { Some("3".to_string()) },
//             accepted,
//             confidence: 0.9,
//             timestamp: Utc::now(),
//         }
//     }
//
//     // ─── Persistence: JSONL round-trip ────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_upsert_and_reload() {
//         let (mut store, _dir) = create_test_store().await;
//         let record = make_record("r001", "todo", true);
//         store.upsert(record.clone()).await.unwrap();
//
//         // Reload from disk
//         let path = store.file_path().to_path_buf();
//         let mut store2 = OutcomeStore::new(path);
//         store2.ensure_loaded().await.unwrap();
//
//         let all = store2.get_all_outcomes();
//         assert_eq!(all.len(), 1);
//         assert_eq!(all[0].id, "r001");
//         assert_eq!(all[0].tool_name, "todo");
//         assert!(all[0].success);
//     }
//
//     #[tokio::test]
//     async fn test_upsert_overwrites_existing_id() {
//         let (mut store, _dir) = create_test_store().await;
//         store.upsert(make_record("r001", "todo", true)).await.unwrap();
//         store.upsert(make_record("r001", "todo", false)).await.unwrap(); // overwrite
//
//         let all = store.get_all_outcomes();
//         assert_eq!(all.len(), 1, "id dedup: only 1 record");
//         assert!(!all[0].success, "should reflect the latest upsert");
//     }
//
//     #[tokio::test]
//     async fn test_delete_tombstone_removes_record() {
//         let (mut store, _dir) = create_test_store().await;
//         store.upsert(make_record("r001", "todo", true)).await.unwrap();
//         store.delete("r001").await.unwrap();
//
//         let all = store.get_all_outcomes();
//         assert!(all.is_empty(), "deleted record should not appear");
//     }
//
//     #[tokio::test]
//     async fn test_enrichment_feedback_appended_and_retrieved() {
//         let (mut store, _dir) = create_test_store().await;
//         let fb = make_feedback("task-001", "priority", true);
//         store.append_feedback(fb).await.unwrap();
//
//         let all = store.get_all_feedback();
//         assert_eq!(all.len(), 1);
//         assert_eq!(all[0].task_id, "task-001");
//         assert!(all[0].accepted);
//     }
//
//     // ─── Compaction ───────────────────────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_compact_reduces_file_size() {
//         let (mut store, _dir) = create_test_store().await;
//         // Write 105 upserts (auto-compact triggers at journal_len > index.len() + 100)
//         for i in 0..105 {
//             store.upsert(make_record(&format!("r{:03}", i), "todo", true)).await.unwrap();
//             // Also overwrite some to create stale journal entries
//             if i % 2 == 0 {
//                 store.upsert(make_record(&format!("r{:03}", i), "read_file", false)).await.unwrap();
//             }
//         }
//
//         let size_before = std::fs::metadata(store.file_path()).unwrap().len();
//         store.compact().await.unwrap();
//         let size_after = std::fs::metadata(store.file_path()).unwrap().len();
//
//         assert!(size_after < size_before, "compacted file must be smaller");
//
//         // Verify data integrity after compaction
//         let path = store.file_path().to_path_buf();
//         let mut store2 = OutcomeStore::new(path);
//         store2.ensure_loaded().await.unwrap();
//         assert_eq!(store2.get_all_outcomes().len(), 105);
//     }
//
//     #[tokio::test]
//     async fn test_atomic_compaction_uses_tmp_then_rename() {
//         // Verifies the .jsonl.tmp file is cleaned up after compaction.
//         // (No leftover .tmp files — crash atomicity guarantee)
//         let (mut store, _dir) = create_test_store().await;
//         for i in 0..10 {
//             store.upsert(make_record(&format!("r{}", i), "todo", true)).await.unwrap();
//         }
//         store.compact().await.unwrap();
//
//         let tmp_path = format!("{}.tmp", store.file_path().to_str().unwrap());
//         assert!(
//             !std::path::Path::new(&tmp_path).exists(),
//             ".jsonl.tmp must be removed after successful compaction"
//         );
//     }
//
//     // ─── Corrupted data recovery ──────────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_corrupted_line_is_skipped() {
//         let temp_dir = TempDir::new().unwrap();
//         let path = temp_dir.path().join("outcomes.jsonl");
//         std::fs::write(&path, b"NOT JSON\n").unwrap();
//
//         let mut store = OutcomeStore::new(path);
//         // Should not panic
//         store.ensure_loaded().await.unwrap();
//         assert_eq!(store.get_all_outcomes().len(), 0);
//     }
//
//     #[tokio::test]
//     async fn test_missing_file_loads_as_empty() {
//         let temp_dir = TempDir::new().unwrap();
//         let path = temp_dir.path().join("nonexistent.jsonl");
//         let mut store = OutcomeStore::new(path);
//         store.ensure_loaded().await.unwrap();
//         assert_eq!(store.get_all_outcomes().len(), 0);
//         assert_eq!(store.get_all_feedback().len(), 0);
//     }
// }

// ═══════════════════════════════════════════════════════════════════════════
// FILE: crates/agent/src/learning/recorder.rs
// Copy the block below to the bottom of recorder.rs
// ═══════════════════════════════════════════════════════════════════════════
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::learning::{OutcomeStore, types::ExecutionMode};
//     use std::sync::Arc;
//     use tokio::sync::RwLock;
//     use tempfile::TempDir;
//
//     async fn create_recorder() -> (OutcomeRecorder, Arc<RwLock<OutcomeStore>>, TempDir) {
//         let temp_dir = TempDir::new().unwrap();
//         let path = temp_dir.path().join("outcomes.jsonl");
//         let store = Arc::new(RwLock::new(OutcomeStore::new(path)));
//         let recorder = OutcomeRecorder::new(Arc::clone(&store));
//         (recorder, store, temp_dir)
//     }
//
//     // ─── Privacy: privacy-by-omission ────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_session_key_suffix_is_hashed() {
//         let (recorder, store, _dir) = create_recorder().await;
//         recorder.record_tool_outcome(
//             "todo",
//             true,
//             None,
//             42,
//             None,
//             ExecutionMode::Chat,
//             "telegram:user99999",
//         ).await;
//
//         let guard = store.read().await;
//         let outcomes = guard.get_all_outcomes();
//         assert_eq!(outcomes.len(), 1);
//         assert!(!outcomes[0].session_key.contains("user99999"),
//             "raw session suffix must not appear in stored key");
//         assert!(outcomes[0].session_key.starts_with("telegram:"),
//             "channel prefix must be preserved");
//     }
//
//     #[tokio::test]
//     async fn test_hashing_is_deterministic() {
//         // Same session_key input → same hashed output
//         let hash1 = hash_session_suffix("telegram:user99999");
//         let hash2 = hash_session_suffix("telegram:user99999");
//         assert_eq!(hash1, hash2, "hashing must be deterministic");
//     }
//
//     #[tokio::test]
//     async fn test_different_inputs_produce_different_hashes() {
//         let hash1 = hash_session_suffix("telegram:user1");
//         let hash2 = hash_session_suffix("telegram:user2");
//         assert_ne!(hash1, hash2, "different inputs should produce different hashes");
//     }
//
//     // ─── Outcome construction ─────────────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_record_tool_outcome_success_stored() {
//         let (recorder, store, _dir) = create_recorder().await;
//         recorder.record_tool_outcome("todo", true, None, 100, None, ExecutionMode::Chat, "test:s1").await;
//
//         let guard = store.read().await;
//         let outcomes = guard.get_all_outcomes();
//         assert_eq!(outcomes.len(), 1);
//         assert!(outcomes[0].success);
//         assert_eq!(outcomes[0].tool_name, "todo");
//         assert_eq!(outcomes[0].duration_ms, 100);
//         assert!(outcomes[0].error_category.is_none());
//     }
//
//     #[tokio::test]
//     async fn test_record_tool_outcome_failure_has_category() {
//         let (recorder, store, _dir) = create_recorder().await;
//         recorder.record_tool_outcome(
//             "shell",
//             false,
//             Some("permission"),
//             200,
//             None,
//             ExecutionMode::Chat,
//             "test:s1",
//         ).await;
//
//         let guard = store.read().await;
//         let outcomes = guard.get_all_outcomes();
//         assert!(!outcomes[0].success);
//         assert_eq!(outcomes[0].error_category.as_deref(), Some("permission"));
//     }
//
//     #[tokio::test]
//     async fn test_plan_step_mode_recorded_correctly() {
//         let (recorder, store, _dir) = create_recorder().await;
//         let mode = ExecutionMode::PlanStep {
//             plan_id: "plan-001".to_string(),
//             step_index: 0,
//         };
//         recorder.record_tool_outcome("todo", true, None, 50, None, mode, "test:s1").await;
//
//         let guard = store.read().await;
//         let outcomes = guard.get_all_outcomes();
//         assert!(matches!(
//             &outcomes[0].execution_mode,
//             ExecutionMode::PlanStep { plan_id, step_index: 0 } if plan_id == "plan-001"
//         ));
//     }
//
//     // ─── Enrichment feedback handler ──────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_record_feedback_stored_in_outcome_store() {
//         use tools::learning_feedback::EnrichmentFeedbackEntry;
//         use chrono::Utc;
//         let (recorder, store, _dir) = create_recorder().await;
//
//         let feedback = EnrichmentFeedbackEntry {
//             task_id: "task-001".to_string(),
//             field: "priority".to_string(),
//             suggested_value: "1".to_string(),
//             actual_value: None,
//             accepted: true,
//             confidence: 0.85,
//             timestamp: Utc::now(),
//         };
//         recorder.record_feedback(feedback).await.unwrap();
//
//         let guard = store.read().await;
//         let all_fb = guard.get_all_feedback();
//         assert_eq!(all_fb.len(), 1);
//         assert_eq!(all_fb[0].task_id, "task-001");
//         assert!(all_fb[0].accepted);
//     }
// }

// ═══════════════════════════════════════════════════════════════════════════
// FILE: crates/agent/src/learning/analyzer.rs
// Copy the block below to the bottom of analyzer.rs
// ═══════════════════════════════════════════════════════════════════════════
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::learning::types::*;
//     use chrono::Utc;
//
//     fn make_outcome(tool: &str, success: bool, confidence: f32) -> OutcomeRecord {
//         OutcomeRecord {
//             id: uuid::Uuid::new_v4().to_string(),
//             session_key: "telegram:abc".to_string(),
//             tool_name: tool.to_string(),
//             success,
//             error_category: None,
//             duration_ms: 100,
//             confidence_score: Some(confidence),
//             confidence_dimensions: None,
//             execution_mode: ExecutionMode::Chat,
//             created_at: Utc::now(),
//         }
//     }
//
//     fn make_feedback(field: &str, accepted: bool) -> EnrichmentFeedbackEntry {
//         EnrichmentFeedbackEntry {
//             task_id: uuid::Uuid::new_v4().to_string(),
//             field: field.to_string(),
//             suggested_value: "1".to_string(),
//             actual_value: if accepted { None } else { Some("3".to_string()) },
//             accepted,
//             confidence: 0.9,
//             timestamp: Utc::now(),
//         }
//     }
//
//     // ─── Band boundaries ──────────────────────────────────────────────────
//
//     #[test]
//     fn test_analyzer_produces_correct_band_count() {
//         let outcomes: Vec<OutcomeRecord> = [0.1, 0.4, 0.6, 0.75, 0.9]
//             .iter()
//             .map(|&c| make_outcome("todo", true, c))
//             .collect();
//
//         let result = LearningAnalyzer::analyze(&outcomes, &[]);
//         let stats = result.per_tool_stats.get("todo").unwrap();
//         // Expect 5 bands: [0-0.3], [0.3-0.5], [0.5-0.7], [0.7-0.85], [0.85-1.0]
//         assert_eq!(stats.success_rate_by_confidence_band.len(), 5);
//     }
//
//     #[test]
//     fn test_band_boundaries_are_exact() {
//         let outcomes = vec![make_outcome("todo", true, 0.1)]; // falls in [0.0-0.3]
//         let result = LearningAnalyzer::analyze(&outcomes, &[]);
//         let stats = result.per_tool_stats.get("todo").unwrap();
//         let first_band = &stats.success_rate_by_confidence_band[0];
//         assert!((first_band.lower - 0.0).abs() < f32::EPSILON);
//         assert!((first_band.upper - 0.3).abs() < f32::EPSILON);
//     }
//
//     // ─── Threshold suggestion ─────────────────────────────────────────────
//
//     #[test]
//     fn test_suggested_threshold_within_spec_bounds() {
//         let outcomes: Vec<OutcomeRecord> = (0..60)
//             .map(|i| make_outcome("todo", i % 2 == 0, 0.5 + (i as f32 / 200.0)))
//             .collect();
//
//         let result = LearningAnalyzer::analyze(&outcomes, &[]);
//         assert!(result.suggested_threshold >= 0.4, "must not go below 0.4");
//         assert!(result.suggested_threshold <= 0.9, "must not exceed 0.9");
//     }
//
//     #[test]
//     fn test_all_success_suggests_lower_threshold() {
//         // If everything succeeds regardless of confidence, optimal threshold is lower
//         let outcomes: Vec<OutcomeRecord> = (0..60)
//             .map(|i| make_outcome("todo", true, i as f32 / 60.0))
//             .collect();
//         let result = LearningAnalyzer::analyze(&outcomes, &[]);
//         // With 100% success at all confidence levels, threshold should trend toward lower
//         assert!(result.suggested_threshold < 0.75,
//             "all-success outcomes imply a lower threshold is safe");
//     }
//
//     #[test]
//     fn test_all_failure_suggests_higher_threshold() {
//         // If everything fails at low confidence, threshold should be higher
//         let outcomes: Vec<OutcomeRecord> = (0..60)
//             .map(|i| {
//                 let c = i as f32 / 60.0;
//                 make_outcome("todo", c > 0.7, c) // only succeed at high confidence
//             })
//             .collect();
//         let result = LearningAnalyzer::analyze(&outcomes, &[]);
//         assert!(result.suggested_threshold >= 0.6,
//             "failure at low confidence should raise suggested threshold");
//     }
//
//     // ─── Enrichment stats ─────────────────────────────────────────────────
//
//     #[test]
//     fn test_enrichment_acceptance_rate() {
//         let feedback: Vec<EnrichmentFeedbackEntry> = (0..10)
//             .map(|i| make_feedback("priority", i < 7)) // 7 accepted, 3 overridden
//             .collect();
//
//         let result = LearningAnalyzer::analyze(&[], &feedback);
//         let stats = &result.enrichment_stats;
//         assert_eq!(stats.total_suggestions, 10);
//         assert_eq!(stats.accepted_count, 7);
//         assert_eq!(stats.overridden_count, 3);
//         assert!((stats.acceptance_rate - 0.7).abs() < 0.001);
//     }
//
//     #[test]
//     fn test_per_field_enrichment_stats() {
//         let feedback = vec![
//             make_feedback("priority", true),
//             make_feedback("priority", false),
//             make_feedback("estimated_minutes", true),
//             make_feedback("estimated_minutes", true),
//         ];
//         let result = LearningAnalyzer::analyze(&[], &feedback);
//         let priority = result.enrichment_stats.per_field.get("priority").unwrap();
//         let minutes = result.enrichment_stats.per_field.get("estimated_minutes").unwrap();
//         assert_eq!(priority.total, 2);
//         assert!((priority.acceptance_rate - 0.5).abs() < 0.001);
//         assert_eq!(minutes.total, 2);
//         assert!((minutes.acceptance_rate - 1.0).abs() < 0.001);
//     }
//
//     #[test]
//     fn test_per_tool_stats_aggregated() {
//         let outcomes = vec![
//             make_outcome("todo", true, 0.8),
//             make_outcome("todo", false, 0.6),
//             make_outcome("read_file", true, 0.9),
//         ];
//         let result = LearningAnalyzer::analyze(&outcomes, &[]);
//         assert_eq!(result.per_tool_stats.len(), 2);
//         let todo_stats = result.per_tool_stats.get("todo").unwrap();
//         assert_eq!(todo_stats.total_calls, 2);
//         assert_eq!(todo_stats.success_count, 1);
//         assert_eq!(result.total_outcomes, 3);
//     }
//
//     #[test]
//     fn test_empty_outcomes_returns_sensible_defaults() {
//         let result = LearningAnalyzer::analyze(&[], &[]);
//         assert_eq!(result.total_outcomes, 0);
//         assert!(result.per_tool_stats.is_empty());
//         // suggested_threshold should be a reasonable default (e.g., 0.7)
//         assert!(result.suggested_threshold >= 0.4);
//         assert!(result.suggested_threshold <= 0.9);
//     }
// }

// ═══════════════════════════════════════════════════════════════════════════
// FILE: crates/agent/src/learning/adaptive.rs
// Copy the block below to the bottom of adaptive.rs
// ═══════════════════════════════════════════════════════════════════════════
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::learning::types::*;
//     use chrono::Utc;
//     use tempfile::TempDir;
//
//     async fn create_adaptive(initial: f32) -> (AdaptiveThresholds, TempDir) {
//         let temp_dir = TempDir::new().unwrap();
//         let state_path = temp_dir.path().join("learning_state.json");
//         let adaptive = AdaptiveThresholds::load(state_path, initial).await;
//         (adaptive, temp_dir)
//     }
//
//     fn make_analysis(total: usize, suggested: f32) -> AnalysisResult {
//         AnalysisResult {
//             computed_at: Utc::now(),
//             total_outcomes: total,
//             per_tool_stats: Default::default(),
//             suggested_threshold: suggested,
//             threshold_confidence: 0.8,
//             enrichment_stats: EnrichmentStats::default(),
//         }
//     }
//
//     // ─── Cold start protection ────────────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_cold_start_returns_none_below_50() {
//         let (mut adaptive, _dir) = create_adaptive(0.7).await;
//         let analysis = make_analysis(49, 0.5); // below MIN_OUTCOMES_FOR_ADAPTATION
//         let result = adaptive.apply_analysis(&analysis);
//         assert!(result.is_none(), "must not adapt with fewer than 50 outcomes");
//     }
//
//     #[tokio::test]
//     async fn test_adaptation_allowed_at_50() {
//         let (mut adaptive, _dir) = create_adaptive(0.7).await;
//         let analysis = make_analysis(50, 0.5); // exactly at threshold, big suggested change
//         let result = adaptive.apply_analysis(&analysis);
//         // Should return Some (maybe not a full drop but some adjustment)
//         assert!(result.is_some(), "must adapt once minimum is reached");
//     }
//
//     // ─── Bounds clamping ──────────────────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_threshold_never_drops_below_min() {
//         let (mut adaptive, _dir) = create_adaptive(0.42).await;
//         let analysis = make_analysis(100, 0.1); // suggest far below min
//         let result = adaptive.apply_analysis(&analysis);
//         let new_val = result.unwrap_or(adaptive.state.current_threshold);
//         assert!(new_val >= MIN_THRESHOLD,
//             "threshold must stay >= {} (got {})", MIN_THRESHOLD, new_val);
//     }
//
//     #[tokio::test]
//     async fn test_threshold_never_exceeds_max() {
//         let (mut adaptive, _dir) = create_adaptive(0.88).await;
//         let analysis = make_analysis(100, 1.0); // suggest far above max
//         let result = adaptive.apply_analysis(&analysis);
//         let new_val = result.unwrap_or(adaptive.state.current_threshold);
//         assert!(new_val <= MAX_THRESHOLD,
//             "threshold must stay <= {} (got {})", MAX_THRESHOLD, new_val);
//     }
//
//     // ─── Step limit ───────────────────────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_single_step_max_is_0_05() {
//         let (mut adaptive, _dir) = create_adaptive(0.7).await;
//         let analysis = make_analysis(100, 0.3); // suggest large drop
//         let old = adaptive.state.current_threshold;
//         if let Some(new_val) = adaptive.apply_analysis(&analysis) {
//             assert!(
//                 (new_val - old).abs() <= MAX_THRESHOLD_STEP + f32::EPSILON,
//                 "single step must not exceed {} (change was {})",
//                 MAX_THRESHOLD_STEP, (new_val - old).abs()
//             );
//         }
//     }
//
//     // ─── History tracking ─────────────────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_threshold_change_recorded_in_history() {
//         let (mut adaptive, _dir) = create_adaptive(0.7).await;
//         adaptive.apply_analysis(&make_analysis(100, 0.5)); // triggers change
//         assert!(!adaptive.state.threshold_history.is_empty(),
//             "ThresholdChange should be recorded");
//         let change = &adaptive.state.threshold_history[0];
//         assert!((change.from - 0.7).abs() < f32::EPSILON);
//         assert!(!change.reason.is_empty());
//     }
//
//     // ─── Persistence ──────────────────────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_save_and_reload_preserves_state() {
//         let (mut adaptive, dir) = create_adaptive(0.7).await;
//         let state_path = dir.path().join("learning_state.json");
//
//         // Apply a change and save
//         adaptive.apply_analysis(&make_analysis(100, 0.5));
//         adaptive.save().await.unwrap();
//         let saved_threshold = adaptive.state.current_threshold;
//
//         // Reload
//         let reloaded = AdaptiveThresholds::load(state_path, 0.7).await;
//         assert!((reloaded.state.current_threshold - saved_threshold).abs() < f32::EPSILON,
//             "reloaded threshold must match saved value");
//         assert_eq!(reloaded.state.threshold_history.len(), adaptive.state.threshold_history.len());
//     }
//
//     #[tokio::test]
//     async fn test_load_with_missing_file_uses_initial() {
//         let temp_dir = TempDir::new().unwrap();
//         let path = temp_dir.path().join("nonexistent_state.json");
//         let adaptive = AdaptiveThresholds::load(path, 0.72).await;
//         assert!((adaptive.state.current_threshold - 0.72).abs() < f32::EPSILON);
//         assert!(adaptive.state.threshold_history.is_empty());
//     }
//
//     // ─── No-change idempotency ────────────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_tiny_change_returns_none() {
//         // If suggested threshold is within 0.001 of current, no change is returned
//         let (mut adaptive, _dir) = create_adaptive(0.7).await;
//         let analysis = make_analysis(100, 0.7001); // near-identical
//         let result = adaptive.apply_analysis(&analysis);
//         assert!(result.is_none(), "sub-0.001 change should not be reported");
//     }
// }

// ═══════════════════════════════════════════════════════════════════════════
// FILE: crates/agent/src/learning/service.rs
// Copy the block below to the bottom of service.rs
// ═══════════════════════════════════════════════════════════════════════════
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::learning::{OutcomeStore, adaptive::AdaptiveThresholds};
//     use std::sync::{atomic::{AtomicU32, Ordering}, Arc};
//     use std::time::Duration;
//     use tokio::sync::RwLock;
//     use tempfile::TempDir;
//
//     async fn create_service(temp_dir: &TempDir) -> LearningService {
//         let outcomes_path = temp_dir.path().join("outcomes.jsonl");
//         let state_path = temp_dir.path().join("state.json");
//
//         let store = Arc::new(RwLock::new(OutcomeStore::new(outcomes_path)));
//         let adaptive = Arc::new(RwLock::new(
//             AdaptiveThresholds::load(state_path, 0.7).await
//         ));
//         let threshold = Arc::new(AtomicU32::new(0.7_f32.to_bits()));
//
//         LearningService::new(store, adaptive, threshold, Duration::from_secs(9999))
//     }
//
//     // ─── Lifecycle ────────────────────────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_service_start_and_stop_does_not_panic() {
//         let temp_dir = TempDir::new().unwrap();
//         let mut service = create_service(&temp_dir).await;
//         service.start();
//         service.stop().await;
//         // No panic = pass
//     }
//
//     #[tokio::test]
//     async fn test_double_stop_is_safe() {
//         let temp_dir = TempDir::new().unwrap();
//         let mut service = create_service(&temp_dir).await;
//         service.start();
//         service.stop().await;
//         service.stop().await; // second stop must not panic
//     }
//
//     // ─── analyze_now ──────────────────────────────────────────────────────
//
//     #[tokio::test]
//     async fn test_analyze_now_returns_result() {
//         let temp_dir = TempDir::new().unwrap();
//         let service = create_service(&temp_dir).await;
//         let result = service.analyze_now().await;
//         assert!(result.is_ok(), "analyze_now must return Ok: {:?}", result.err());
//         let analysis = result.unwrap();
//         // Empty store → 0 outcomes
//         assert_eq!(analysis.total_outcomes, 0);
//     }
//
//     #[tokio::test]
//     async fn test_analyze_now_while_background_running() {
//         let temp_dir = TempDir::new().unwrap();
//         let mut service = create_service(&temp_dir).await;
//         // Start with short interval so background runs
//         service.start();
//         tokio::time::sleep(Duration::from_millis(10)).await;
//         // analyze_now should not deadlock or panic
//         let result = service.analyze_now().await;
//         assert!(result.is_ok());
//         service.stop().await;
//     }
// }

// ═══════════════════════════════════════════════════════════════════════════
// FILE: crates/tools/src/learning_feedback.rs
// Copy the block below to the bottom of learning_feedback.rs
// ═══════════════════════════════════════════════════════════════════════════
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use chrono::Utc;
//
//     #[test]
//     fn test_enrichment_feedback_entry_serde() {
//         let entry = EnrichmentFeedbackEntry {
//             task_id: "t001".to_string(),
//             field: "priority".to_string(),
//             suggested_value: "1".to_string(),
//             actual_value: Some("3".to_string()),
//             accepted: false,
//             confidence: 0.85,
//             timestamp: Utc::now(),
//         };
//         let json = serde_json::to_string(&entry).unwrap();
//         let back: EnrichmentFeedbackEntry = serde_json::from_str(&json).unwrap();
//         assert_eq!(back.task_id, "t001");
//         assert!(!back.accepted);
//         assert!((back.confidence - 0.85).abs() < f64::EPSILON);
//     }
//
//     #[test]
//     fn test_feedback_accepted_when_actual_is_none() {
//         // When actual_value is None, the suggestion was kept as-is → accepted
//         let entry = EnrichmentFeedbackEntry {
//             task_id: "t002".to_string(),
//             field: "estimated_minutes".to_string(),
//             suggested_value: "30".to_string(),
//             actual_value: None, // user kept the suggestion
//             accepted: true,
//             confidence: 0.9,
//             timestamp: Utc::now(),
//         };
//         assert!(entry.accepted);
//         assert!(entry.actual_value.is_none());
//     }
// }

// ═══════════════════════════════════════════════════════════════════════════
// FILE: crates/agent/src/confidence/evaluator.rs
// Add these tests to the EXISTING #[cfg(test)] mod tests block
// ═══════════════════════════════════════════════════════════════════════════
//
//     #[test]
//     fn test_atomic_threshold_is_readable() {
//         // After task #12: ConfidenceEvaluator uses Arc<AtomicU32>
//         let evaluator = ConfidenceEvaluator::new(0.65);
//         assert!((evaluator.threshold() - 0.65).abs() < f32::EPSILON);
//     }
//
//     #[test]
//     fn test_threshold_handle_allows_external_update() {
//         // After task #12: threshold_handle() shares the atomic with LearningService
//         let evaluator = ConfidenceEvaluator::new(0.65);
//         let handle = evaluator.threshold_handle();
//         // Simulate LearningService updating the threshold
//         handle.store(0.72_f32.to_bits(), std::sync::atomic::Ordering::SeqCst);
//         assert!((evaluator.threshold() - 0.72).abs() < f32::EPSILON,
//             "evaluator must reflect the externally updated threshold");
//     }
//
//     #[test]
//     fn test_threshold_clamped_to_valid_range() {
//         let below = ConfidenceEvaluator::new(-0.5);
//         assert!((below.threshold() - 0.0).abs() < f32::EPSILON);
//         let above = ConfidenceEvaluator::new(1.5);
//         assert!((above.threshold() - 1.0).abs() < f32::EPSILON);
//     }
