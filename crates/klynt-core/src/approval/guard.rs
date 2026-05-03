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
}

pub async fn evaluate<'a>(ctx: GuardCtx<'a>, tool: &str, payload: &str) -> ApprovalDecision {
    let mut audit = LayerOutcomeAudit {
        privacy_passed: true,
        layer1: "skipped".into(),
        layer2: "skipped".into(),
        layer3: "skipped".into(),
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
    audit.layer1 = format_layer_outcome(&l1);

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
    audit.layer2 = format_layer2_outcome(&l2);
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
        let args_json = ctx.args.as_ref().map(|v| v.to_string()).unwrap_or_default();
        let hash = crate::approval::layer3::args_hash_for_relevance(tool, &args_json);
        let summary = repo
            .summary(tool, &hash, &ctx.repo_id)
            .await
            .unwrap_or_default();
        match crate::approval::layer3::evaluate(&cfg, &summary, ctx.now_unix) {
            crate::approval::layer3::Layer3Outcome::AutoAllow { reason } => {
                audit.layer3 = format!("auto-allow: {reason}");
                let decision = ApprovalDecision::auto_allow(ApprovalLayer::Layer3Mirror, reason);
                emit_pair(&ctx, tool, payload, &decision, false, Some(&summary)).await;
                return decision;
            }
            crate::approval::layer3::Layer3Outcome::Ask { reason } => {
                audit.layer3 = format!("ask: {reason}");
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
                audit.layer3 = "deferred: not enough history".into();
            }
        }
    } else {
        audit.layer3 = "skipped: no history repo".into();
    }

    match merged {
        ApprovalDecision::Auto { .. } => {
            emit_pair(&ctx, tool, payload, &merged, false, None).await;
            merged
        }
        ApprovalDecision::Ask { .. } => {
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

fn format_layer_outcome(d: &ApprovalDecision) -> String {
    match d {
        ApprovalDecision::Auto {
            allowed: true,
            rule_matched,
            ..
        } => format!(
            "allowed: {}",
            rule_matched.as_deref().unwrap_or("?")
        ),
        ApprovalDecision::Auto {
            allowed: false,
            rule_matched,
            ..
        } => format!(
            "denied: {}",
            rule_matched.as_deref().unwrap_or("?")
        ),
        ApprovalDecision::Ask { reason, .. } => format!("ask: {reason}"),
        _ => "?".into(),
    }
}

fn format_layer2_outcome(d: &ExecDecision) -> String {
    match d {
        ExecDecision::Allow => "allowed".into(),
        ExecDecision::Forbid => "denied".into(),
        ExecDecision::Ask => "ask".into(),
        ExecDecision::FallThrough => "deferred: no rule".into(),
    }
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
