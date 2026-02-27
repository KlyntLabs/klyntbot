//! Serialization tests for AgentEvent.
//!
//! Verifies that every AgentEvent variant serializes with:
//! - An internally-tagged `"type"` field using camelCase variant name
//! - Field names in camelCase (rename_all = "camelCase")
//!
//! Expected shapes (per architecture plan §5 and UX spec §1.1):
//!   ContentChunk  → { "type": "contentChunk",  "data": "..." }
//!   Done          → { "type": "done",           "content": "..." }
//!   Error         → { "type": "error",          "message": "..." }
//!   ToolStart     → { "type": "toolStart",      "name": "...", "args": {...} }
//!   ToolEnd       → { "type": "toolEnd",        "name": "...", "success": bool, "durationMs": N }
//!   IterationStart→ { "type": "iterationStart", "iteration": N, "max": N }
//!   ClassificationComplete → { "type": "classificationComplete", "strategy": "...", ... }
//!   ContextAssembled       → { "type": "contextAssembled",       "totalTokens": N, ... }
//!   ExecutionStarted       → { "type": "executionStarted",       "engine": "...", ... }
//!   ConfidenceAssessed     → { "type": "confidenceAssessed",     "score": N, "action": "..." }
//!   PlanStepCompleted      → { "type": "planStepCompleted",      "planId": "...", ... }
//!   PlanCompleted          → { "type": "planCompleted",          "planId": "...", "summary": "..." }

#[cfg(test)]
mod tests {
    use crate::events::AgentEvent;

    fn serialize(event: &AgentEvent) -> serde_json::Value {
        serde_json::to_value(event).expect("AgentEvent should serialize")
    }

    // ── ContentChunk ──────────────────────────────────────────────────────────

    #[test]
    fn content_chunk_serializes_with_data_field() {
        // AC-3.1: ContentChunk { data } serializes as {"type":"contentChunk","data":"hello"}
        let event = AgentEvent::ContentChunk {
            data: "hello".to_string(),
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "contentChunk");
        assert_eq!(v["data"], "hello");
    }

    #[test]
    fn content_chunk_type_tag_is_camel_case() {
        // AC-3.7: The "type" field value must be "contentChunk" (camelCase)
        let event = AgentEvent::ContentChunk {
            data: "x".to_string(),
        };
        let v = serialize(&event);
        let tag = v["type"].as_str().unwrap();
        assert_eq!(tag, "contentChunk");
        assert!(!tag.contains('_'), "type tag must not contain underscore");
    }

    // ── Done ──────────────────────────────────────────────────────────────────

    #[test]
    fn done_serializes_with_content_field() {
        // AC-3.2: Done { content } serializes as {"type":"done","content":"final response"}
        let event = AgentEvent::Done {
            content: "final response".to_string(),
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "done");
        assert_eq!(v["content"], "final response");
    }

    // ── Error ─────────────────────────────────────────────────────────────────

    #[test]
    fn error_serializes_with_message_field() {
        // AC-3.3: Error { message } serializes as {"type":"error","message":"what went wrong"}
        let event = AgentEvent::Error {
            message: "what went wrong".to_string(),
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "error");
        assert_eq!(v["message"], "what went wrong");
    }

    // ── ToolStart ─────────────────────────────────────────────────────────────

    #[test]
    fn tool_start_serializes_with_name_and_args() {
        // AC-3.4: ToolStart { name, args } serializes as {"type":"toolStart","name":"todo","args":{...}}
        let event = AgentEvent::ToolStart {
            name: "todo".to_string(),
            args: serde_json::json!({"action": "add"}),
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "toolStart");
        assert_eq!(v["name"], "todo");
        assert_eq!(v["args"]["action"], "add");
    }

    #[test]
    fn tool_start_type_tag_is_camel_case() {
        // AC-3.7: Type tag is "toolStart" not "tool_start"
        let event = AgentEvent::ToolStart {
            name: "x".to_string(),
            args: serde_json::Value::Null,
        };
        let v = serialize(&event);
        let tag = v["type"].as_str().unwrap();
        assert_eq!(tag, "toolStart");
    }

    // ── ToolEnd ───────────────────────────────────────────────────────────────

    #[test]
    fn tool_end_serializes_with_duration_ms_camel_case() {
        // AC-3.5: ToolEnd { name, success, duration_ms } serializes with camelCase field
        // duration_ms → "durationMs" in JSON
        let event = AgentEvent::ToolEnd {
            name: "todo".to_string(),
            success: true,
            duration_ms: 42,
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "toolEnd");
        assert_eq!(v["name"], "todo");
        assert_eq!(v["success"], true);
        assert_eq!(v["durationMs"], 42);
        // Confirm snake_case key is absent
        assert!(
            v.get("duration_ms").is_none(),
            "should not have duration_ms key"
        );
    }

    #[test]
    fn tool_end_type_tag_is_camel_case() {
        // AC-3.7: Type tag is "toolEnd" not "tool_end"
        let event = AgentEvent::ToolEnd {
            name: "x".to_string(),
            success: false,
            duration_ms: 0,
        };
        let v = serialize(&event);
        assert_eq!(v["type"].as_str().unwrap(), "toolEnd");
    }

    // ── IterationStart ────────────────────────────────────────────────────────

    #[test]
    fn iteration_start_serializes() {
        // AC-3.7: {"type":"iterationStart","iteration":1,"max":5}
        let event = AgentEvent::IterationStart {
            iteration: 1,
            max: 5,
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "iterationStart");
        assert_eq!(v["iteration"], 1);
        assert_eq!(v["max"], 5);
    }

    // ── ClassificationComplete ────────────────────────────────────────────────

    #[test]
    fn classification_complete_serializes_all_fields_camel_case() {
        // AC-3.6: ClassificationComplete { strategy, confidence, source, duration_ms }
        // duration_ms → "durationMs"; type tag → "classificationComplete"
        let event = AgentEvent::ClassificationComplete {
            strategy: "direct".to_string(),
            confidence: 0.9,
            source: "heuristic".to_string(),
            duration_ms: 10,
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "classificationComplete");
        assert_eq!(v["strategy"], "direct");
        assert_eq!(v["durationMs"], 10);
        assert!(v.get("duration_ms").is_none());
    }

    // ── ContextAssembled ──────────────────────────────────────────────────────

    #[test]
    fn context_assembled_serializes_camel_case() {
        // AC-3.6: ContextAssembled { total_tokens, budget, duration_ms }
        // total_tokens → "totalTokens", duration_ms → "durationMs"
        let event = AgentEvent::ContextAssembled {
            total_tokens: 1024,
            budget: 4096,
            duration_ms: 5,
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "contextAssembled");
        assert_eq!(v["totalTokens"], 1024);
        assert_eq!(v["budget"], 4096);
        assert_eq!(v["durationMs"], 5);
        assert!(v.get("total_tokens").is_none());
    }

    // ── ExecutionStarted ──────────────────────────────────────────────────────

    #[test]
    fn execution_started_serializes_camel_case() {
        // AC-3.6: ExecutionStarted { engine, max_iterations }
        // max_iterations → "maxIterations"
        let event = AgentEvent::ExecutionStarted {
            engine: "react-plus".to_string(),
            max_iterations: 20,
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "executionStarted");
        assert_eq!(v["engine"], "react-plus");
        assert_eq!(v["maxIterations"], 20);
        assert!(v.get("max_iterations").is_none());
    }

    // ── ConfidenceAssessed ────────────────────────────────────────────────────

    #[test]
    fn confidence_assessed_serializes() {
        // AC-3.7: {"type":"confidenceAssessed","score":0.85,"action":"respond"}
        let event = AgentEvent::ConfidenceAssessed {
            score: 0.85,
            action: "respond".to_string(),
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "confidenceAssessed");
        assert_eq!(v["action"], "respond");
    }

    // ── PlanStepCompleted ─────────────────────────────────────────────────────

    #[test]
    fn plan_step_completed_serializes_camel_case() {
        // AC-3.7: PlanStepCompleted { plan_id, step_index, result }
        // plan_id → "planId", step_index → "stepIndex"
        let id = uuid::Uuid::new_v4();
        let event = AgentEvent::PlanStepCompleted {
            plan_id: id,
            step_index: 2,
            result: "done".to_string(),
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "planStepCompleted");
        assert_eq!(v["planId"], id.to_string());
        assert_eq!(v["stepIndex"], 2);
        assert!(v.get("plan_id").is_none());
        assert!(v.get("step_index").is_none());
    }

    // ── PlanCompleted ─────────────────────────────────────────────────────────

    #[test]
    fn plan_completed_serializes_camel_case() {
        // AC-3.7: PlanCompleted { plan_id, summary }
        // plan_id → "planId"
        let id = uuid::Uuid::new_v4();
        let event = AgentEvent::PlanCompleted {
            plan_id: id,
            summary: "all steps done".to_string(),
        };
        let v = serialize(&event);
        assert_eq!(v["type"], "planCompleted");
        assert_eq!(v["planId"], id.to_string());
        assert_eq!(v["summary"], "all steps done");
        assert!(v.get("plan_id").is_none());
    }

    // ── Cross-cutting ─────────────────────────────────────────────────────────

    fn all_variants() -> Vec<AgentEvent> {
        let id = uuid::Uuid::new_v4();
        vec![
            AgentEvent::ContentChunk {
                data: "chunk".to_string(),
            },
            AgentEvent::ToolStart {
                name: "tool".to_string(),
                args: serde_json::Value::Null,
            },
            AgentEvent::ToolEnd {
                name: "tool".to_string(),
                success: true,
                duration_ms: 1,
            },
            AgentEvent::IterationStart {
                iteration: 1,
                max: 5,
            },
            AgentEvent::ClassificationComplete {
                strategy: "s".to_string(),
                confidence: 0.5,
                source: "src".to_string(),
                duration_ms: 1,
            },
            AgentEvent::ContextAssembled {
                total_tokens: 100,
                budget: 1000,
                duration_ms: 1,
            },
            AgentEvent::ExecutionStarted {
                engine: "e".to_string(),
                max_iterations: 5,
            },
            AgentEvent::Done {
                content: "done".to_string(),
            },
            AgentEvent::ConfidenceAssessed {
                score: 0.8,
                action: "act".to_string(),
            },
            AgentEvent::Error {
                message: "err".to_string(),
            },
            AgentEvent::PlanStepCompleted {
                plan_id: id,
                step_index: 0,
                result: "ok".to_string(),
            },
            AgentEvent::PlanCompleted {
                plan_id: id,
                summary: "done".to_string(),
            },
        ]
    }

    #[test]
    fn all_variants_have_type_field() {
        // AC-3.7: Every variant in the AgentEvent enum, when serialized, includes
        // a "type" field (from #[serde(tag = "type")]).
        for event in all_variants() {
            let v = serialize(&event);
            assert!(v.get("type").is_some(), "Missing 'type' field in: {:?}", v);
        }
    }

    #[test]
    fn all_type_tags_are_camel_case() {
        // AC-3.7: No type tag contains an underscore — all use camelCase.
        for event in all_variants() {
            let v = serialize(&event);
            let tag = v["type"].as_str().unwrap();
            assert!(!tag.contains('_'), "Type tag '{}' contains underscore", tag);
        }
    }

    #[test]
    fn all_field_names_are_camel_case() {
        // AC-3.5/3.6: Serialize all variants and confirm no field key contains underscore.
        for event in all_variants() {
            let v = serialize(&event);
            if let serde_json::Value::Object(map) = &v {
                for key in map.keys() {
                    assert!(
                        !key.contains('_'),
                        "Field key '{}' contains underscore in event: {:?}",
                        key,
                        v
                    );
                }
            }
        }
    }
}
