use super::{
    decision::{ApprovalDecision, ApprovalLayer, LayerOutcomeAudit},
    layer1::Layer1,
    round_trip::{await_decision, PendingApprovalsMap},
};
use crate::privacy::PrivacyGuard;
use bus::DomainEventBus;
use klynt_execpolicy::{Decision as ExecDecision, Policy};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::events::ToolEvent;

pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(600);

pub struct GuardCtx<'a> {
    pub layer1: &'a Layer1,
    pub policy: &'a Policy,
    pub privacy: &'a PrivacyGuard,
    pub pending: &'a Arc<PendingApprovalsMap>,
    pub event_tx: Option<&'a mpsc::Sender<ToolEvent>>,
    pub domain_bus: &'a Arc<DomainEventBus>,
    pub cancel: CancellationToken,
    pub request_id: String,
    pub args: Option<serde_json::Value>,
    pub cwd: Option<String>,
    pub channel: common::tool_channel::Channel,
    pub non_ui_policy: common::tool_channel::NonUiPolicy,
    pub history_repo: Option<std::sync::Arc<storage::repos::CodingApprovalHistoryRepo>>,
    pub repo_id: String,
    pub mirror_learning_enabled: bool,
    pub mirror_min_approvals: u32,
    pub mirror_cooldown_seconds: i64,
    pub now_unix: i64,
    // Phase 4: coding surface context
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
}

pub async fn evaluate<'a>(ctx: GuardCtx<'a>, tool: &str, payload: &str) -> ApprovalDecision {
    let mut audit = LayerOutcomeAudit {
        privacy_passed: true,
        layer1: desktop_shared::coding::LayerOutcome::Skipped {
            reason: "skipped".into(),
        },
        layer2: desktop_shared::coding::LayerOutcome::Skipped {
            reason: "skipped".into(),
        },
        layer3: desktop_shared::coding::LayerOutcome::Skipped {
            reason: "skipped".into(),
        },
    };

    // 0. Privacy guard (non-bypassable)
    let privacy_hit = match tool {
        "bash" => ctx.privacy.bash_command_touches_excluded(payload),
        _ => ctx.privacy.is_excluded(std::path::Path::new(payload)),
    };
    if privacy_hit {
        audit.privacy_passed = false;
        let pat = ctx
            .privacy
            .raw_patterns()
            .first()
            .cloned()
            .unwrap_or_default();
        let d = ApprovalDecision::PrivacyDenied {
            reason: "privacy guard: excludePaths match".into(),
            pattern: pat,
        };
        emit_pair(&ctx, tool, payload, &d, false, None).await;
        return d;
    }

    // 1. Layer 1 declarative
    let l1 = ctx.layer1.evaluate(tool, payload);
    audit.layer1 = layer_outcome_from_decision(&l1);

    // Channel-aware degradation: if Layer1 says "ask" but the channel can't
    // surface an approval card (Telegram/Discord/Slack/Email), fall back to
    // the configured policy.
    let l1 = match l1 {
        ApprovalDecision::Ask { .. } if !ctx.channel.supports_approval_ui() => {
            match ctx.non_ui_policy {
                common::tool_channel::NonUiPolicy::Allow => ApprovalDecision::Auto {
                    allowed: true,
                    layer: ApprovalLayer::Layer1Declarative,
                    reason: format!(
                        "non-UI channel ({:?}) fallback: allow per tools.approvalPolicy.nonUiChannels",
                        ctx.channel
                    ),
                    rule_matched: None,
                },
                common::tool_channel::NonUiPolicy::DenyWithError => ApprovalDecision::Auto {
                    allowed: false,
                    layer: ApprovalLayer::Layer1Declarative,
                    reason: format!(
                        "non-UI channel ({:?}) deny: tool '{}' requires approval; \
                         set tools.approvalPolicy.nonUiChannels = \"allow\" to permit",
                        ctx.channel, tool
                    ),
                    rule_matched: None,
                },
            }
        }
        other => other,
    };

    if matches!(l1, ApprovalDecision::Auto { .. }) {
        emit_pair(&ctx, tool, payload, &l1, false, None).await;
        return l1;
    }

    // 2. Layer 2 Starlark — Plan 2 stub returns FallThrough.
    let argv: Vec<&str> = payload.split_whitespace().collect();
    let l2 = ctx.policy.eval(&argv, None);
    audit.layer2 = layer_outcome_from_exec_decision(&l2);
    let merged: ApprovalDecision = match l2 {
        ExecDecision::Allow => ApprovalDecision::Auto {
            allowed: true,
            layer: ApprovalLayer::Layer2Starlark,
            reason: "layer-2 allow".into(),
            rule_matched: None,
        },
        ExecDecision::Forbid => ApprovalDecision::Auto {
            allowed: false,
            layer: ApprovalLayer::Layer2Starlark,
            reason: "layer-2 forbid".into(),
            rule_matched: None,
        },
        ExecDecision::Ask => ApprovalDecision::ask(ApprovalLayer::Layer2Starlark, "layer-2 ask"),
        ExecDecision::FallThrough => l1,
    };

    // 3. Layer 3 — Mirror-learned (opt-in)
    if let Some(repo) = ctx.history_repo.as_ref() {
        let cfg = crate::approval::layer3::Layer3Config {
            enabled: ctx.mirror_learning_enabled,
            min_approvals: ctx.mirror_min_approvals,
            cooldown_seconds: ctx.mirror_cooldown_seconds,
        };
        // Hash from `ctx.args` when present (structured); otherwise fall back to
        // the raw `payload` we're evaluating so call-sites that don't populate
        // `ctx.args` (e.g. unit tests, simple bash gates) still hash the same
        // shape that record() uses.
        let args_json = ctx
            .args
            .as_ref()
            .map(|v| v.to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| payload.to_string());
        let hash = crate::approval::layer3::args_hash_for_relevance(tool, &args_json);
        let summary = repo
            .summary(tool, &hash, &ctx.repo_id)
            .await
            .unwrap_or_default();
        match crate::approval::layer3::evaluate(&cfg, &summary, ctx.now_unix) {
            crate::approval::layer3::Layer3Outcome::AutoAllow { reason } => {
                audit.layer3 = desktop_shared::coding::LayerOutcome::Allowed {
                    reason: format!("auto-allow: {reason}"),
                    rule_matched: None,
                };
                let decision = ApprovalDecision::auto_allow(ApprovalLayer::Layer3Mirror, reason);
                emit_pair(&ctx, tool, payload, &decision, false, Some(&summary)).await;
                return decision;
            }
            crate::approval::layer3::Layer3Outcome::Ask { reason } => {
                audit.layer3 = desktop_shared::coding::LayerOutcome::Deferred {
                    reason: format!("ask: {reason}"),
                };
                emit_approval_request(&ctx, tool, &audit).await;
                let decision =
                    ApprovalDecision::ask_with_audit(ApprovalLayer::Layer3Mirror, reason, audit);
                emit_pair(&ctx, tool, payload, &decision, true, Some(&summary)).await;
                let user = await_decision(
                    ctx.pending,
                    &ctx.request_id,
                    ctx.cancel.clone(),
                    APPROVAL_TIMEOUT,
                )
                .await;
                emit_resolved(&ctx, &user).await;
                return user;
            }
            crate::approval::layer3::Layer3Outcome::FallThrough => {
                audit.layer3 = desktop_shared::coding::LayerOutcome::Skipped {
                    reason: "deferred: not enough history".into(),
                };
            }
        }
    } else {
        audit.layer3 = desktop_shared::coding::LayerOutcome::Skipped {
            reason: "skipped: no history repo".into(),
        };
    }

    match merged {
        ApprovalDecision::Auto { .. } => {
            emit_pair(&ctx, tool, payload, &merged, false, None).await;
            merged
        }
        ApprovalDecision::Ask { .. } => {
            emit_approval_request(&ctx, tool, &audit).await;
            let decision_with_audit = match merged {
                ApprovalDecision::Ask { layer, reason, .. } => {
                    ApprovalDecision::ask_with_audit(layer, reason, audit)
                }
                other => other,
            };
            emit_pair(&ctx, tool, payload, &decision_with_audit, true, None).await;
            let user = await_decision(
                ctx.pending,
                &ctx.request_id,
                ctx.cancel.clone(),
                APPROVAL_TIMEOUT,
            )
            .await;
            emit_resolved(&ctx, &user).await;
            user
        }
        _ => merged,
    }
}

async fn emit_pair<'a>(
    ctx: &GuardCtx<'a>,
    tool: &str,
    payload: &str,
    decision: &ApprovalDecision,
    requires_user_input: bool,
    mirror_history: Option<&storage::repos::ApprovalHistorySummary>,
) {
    let mut h = Sha256::new();
    h.update(payload.as_bytes());
    let args_hash = format!("{:x}", h.finalize());
    let mirror_history_json = mirror_history.map(|s| {
        serde_json::json!({
            "approval_count": s.approval_count,
            "denial_count": s.denial_count,
        })
    });
    let req = ToolEvent::ApprovalRequested {
        request_id: ctx.request_id.clone(),
        tool: tool.into(),
        args_hash,
        layer: format!("{:?}", layer_of(decision)),
        rule_matched: rule_of(decision),
        mirror_history: mirror_history_json,
        sandbox_summary: String::new(),
        requires_user_input,
        args: ctx.args.clone(),
        cwd: ctx.cwd.clone(),
        layer_reason: Some(reason_of(decision)),
    };
    fan_out_tool_event(ctx.event_tx, Some(ctx.domain_bus), req).await;
    if !requires_user_input {
        emit_resolved(ctx, decision).await;
    }
}

async fn emit_resolved<'a>(ctx: &GuardCtx<'a>, decision: &ApprovalDecision) {
    let res = ToolEvent::ApprovalResolved {
        request_id: ctx.request_id.clone(),
        decision: format!("{:?}", decision),
        decision_reason: reason_of(decision),
        latency_ms: 0,
        persisted_rule: None,
        decided_by: decided_by(decision).into(),
    };
    fan_out_tool_event(ctx.event_tx, Some(ctx.domain_bus), res).await;
}

fn layer_of(d: &ApprovalDecision) -> ApprovalLayer {
    match d {
        ApprovalDecision::Auto { layer, .. } | ApprovalDecision::Ask { layer, .. } => layer.clone(),
        ApprovalDecision::PrivacyDenied { .. } => ApprovalLayer::Privacy,
        _ => ApprovalLayer::DefaultMode,
    }
}
fn rule_of(d: &ApprovalDecision) -> Option<String> {
    if let ApprovalDecision::Auto { rule_matched, .. } = d {
        rule_matched.clone()
    } else {
        None
    }
}
fn reason_of(d: &ApprovalDecision) -> String {
    match d {
        ApprovalDecision::Auto { reason, .. } | ApprovalDecision::Ask { reason, .. } => {
            reason.clone()
        }
        ApprovalDecision::PrivacyDenied { reason, .. } => reason.clone(),
        ApprovalDecision::Cancelled => "cancelled".into(),
        ApprovalDecision::TimedOut => "timeout".into(),
    }
}
fn decided_by(d: &ApprovalDecision) -> &'static str {
    match d {
        ApprovalDecision::Auto { allowed: true, .. } => "auto_allow",
        ApprovalDecision::Auto { allowed: false, .. } => "auto_deny",
        ApprovalDecision::Ask { .. } => "user",
        ApprovalDecision::PrivacyDenied { .. } => "auto_deny",
        ApprovalDecision::Cancelled => "cancelled",
        ApprovalDecision::TimedOut => "timeout",
    }
}

/// Map an `ApprovalDecision` into the frontend `LayerOutcome` type.
fn layer_outcome_from_decision(d: &ApprovalDecision) -> desktop_shared::coding::LayerOutcome {
    match d {
        ApprovalDecision::Auto {
            allowed: true,
            reason: _,
            rule_matched,
            ..
        } => desktop_shared::coding::LayerOutcome::Allowed {
            reason: format!(
                "allowed: {}",
                rule_matched.as_deref().unwrap_or("?")
            ),
            rule_matched: rule_matched.clone(),
        },
        ApprovalDecision::Auto {
            allowed: false,
            reason: _,
            rule_matched,
            ..
        } => desktop_shared::coding::LayerOutcome::Denied {
            reason: format!(
                "denied: {}",
                rule_matched.as_deref().unwrap_or("?")
            ),
            rule_matched: rule_matched.clone(),
        },
        ApprovalDecision::Ask { reason, .. } => {
            desktop_shared::coding::LayerOutcome::Deferred {
                reason: format!("ask: {reason}"),
            }
        }
        _ => desktop_shared::coding::LayerOutcome::Skipped {
            reason: "skipped".into(),
        },
    }
}

/// Map an `ExecDecision` into the frontend `LayerOutcome` type.
fn layer_outcome_from_exec_decision(
    d: &ExecDecision,
) -> desktop_shared::coding::LayerOutcome {
    match d {
        ExecDecision::Allow => desktop_shared::coding::LayerOutcome::Allowed {
            reason: "layer-2 allow".into(),
            rule_matched: None,
        },
        ExecDecision::Forbid => desktop_shared::coding::LayerOutcome::Denied {
            reason: "layer-2 forbid".into(),
            rule_matched: None,
        },
        ExecDecision::Ask => desktop_shared::coding::LayerOutcome::Deferred {
            reason: "layer-2 ask".into(),
        },
        ExecDecision::FallThrough => desktop_shared::coding::LayerOutcome::Skipped {
            reason: "layer-2 fall-through".into(),
        },
    }
}

/// Extract a string argument from the optional JSON args payload.
fn get_arg_str(args: &Option<serde_json::Value>, key: &str) -> Option<String> {
    args.as_ref()
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Emit a typed `ApprovalRequest` event for the coding surface.
///
/// No-op if `event_tx` is None.
pub async fn emit_approval_request(
    ctx: &GuardCtx<'_>,
    tool: &str,
    audit: &LayerOutcomeAudit,
) {
    if ctx.event_tx.is_none() {
        return;
    }
    let thread_id = ctx.thread_id.clone().unwrap_or_default();
    let turn_id = ctx.turn_id.clone().unwrap_or_default();
    let approval_id = ctx.request_id.clone();

    let layer_decisions = desktop_shared::coding::LayerDecisions {
        privacy: if audit.privacy_passed {
            desktop_shared::coding::LayerOutcome::Allowed {
                reason: "passed".into(),
                rule_matched: None,
            }
        } else {
            desktop_shared::coding::LayerOutcome::Denied {
                reason: "privacy guard match".into(),
                rule_matched: None,
            }
        },
        layer1: audit.layer1.clone(),
        layer2: audit.layer2.clone(),
        layer3: audit.layer3.clone(),
    };

    let request = match tool {
        "bash" => desktop_shared::coding::ApprovalRequest::CommandExecution {
            approval_id,
            thread_id,
            turn_id,
            command: get_arg_str(&ctx.args, "command")
                .map(|s| s.split_whitespace().map(String::from).collect())
                .unwrap_or_default(),
            cwd: ctx.cwd.clone().unwrap_or_default(),
            reason: "user approval required".into(),
            proposed_execpolicy_amendment: None,
            layer_decisions,
        },
        _ => desktop_shared::coding::ApprovalRequest::FileChange {
            approval_id,
            thread_id,
            turn_id,
            path: get_arg_str(&ctx.args, "path").unwrap_or_default(),
            diff_unified: get_arg_str(&ctx.args, "diff_unified").unwrap_or_default(),
            write_kind: desktop_shared::coding::WriteKind::Modify,
            layer_decisions,
        },
    };

    let payload = serde_json::to_value(&request).unwrap_or_default();
    let evt = ToolEvent::ApprovalRequest {
        id: ctx.request_id.clone(),
        payload,
    };
    fan_out_tool_event(ctx.event_tx, Some(ctx.domain_bus), evt).await;
}

pub(crate) async fn fan_out_tool_event(
    event_tx: Option<&mpsc::Sender<ToolEvent>>,
    domain_bus: Option<&Arc<DomainEventBus>>,
    evt: ToolEvent,
) {
    if let Some(tx) = event_tx {
        let _ = tx.send(evt.clone()).await;
    }
    if let Some(bus) = domain_bus {
        let payload =
            serde_json::to_value(&evt).unwrap_or_else(|_| serde_json::json!({"type": "unknown"}));
        bus.publish(bus::DomainEvent::Generic {
            kind: "agent_event".into(),
            payload,
        });
    }
}
