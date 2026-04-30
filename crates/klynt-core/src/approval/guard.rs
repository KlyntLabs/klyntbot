use super::{
    decision::{ApprovalDecision, ApprovalLayer},
    layer1::Layer1,
    round_trip::{await_decision, PendingApprovalsMap},
};
use crate::privacy::PrivacyGuard;
use agent::events::AgentEvent;
use bus::DomainEventBus;
use klynt_execpolicy::{Decision as ExecDecision, Policy};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(600);

pub struct GuardCtx<'a> {
    pub layer1: &'a Layer1,
    pub policy: &'a Policy,
    pub privacy: &'a PrivacyGuard,
    pub pending: &'a Arc<PendingApprovalsMap>,
    pub event_tx: Option<&'a mpsc::Sender<AgentEvent>>,
    pub domain_bus: &'a Arc<DomainEventBus>,
    pub cancel: CancellationToken,
    pub request_id: String,
}

pub async fn evaluate<'a>(ctx: GuardCtx<'a>, tool: &str, payload: &str) -> ApprovalDecision {
    // 0. Privacy guard (non-bypassable)
    let privacy_hit = match tool {
        "bash" => ctx.privacy.bash_command_touches_excluded(payload),
        _ => ctx.privacy.is_excluded(std::path::Path::new(payload)),
    };
    if privacy_hit {
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
        emit_pair(&ctx, tool, payload, &d, false).await;
        return d;
    }

    // 1. Layer 1 declarative
    let l1 = ctx.layer1.evaluate(tool, payload);
    if matches!(l1, ApprovalDecision::Auto { .. }) {
        emit_pair(&ctx, tool, payload, &l1, false).await;
        return l1;
    }

    // 2. Layer 2 Starlark — Plan 2 stub returns FallThrough.
    let argv: Vec<&str> = payload.split_whitespace().collect();
    let l2 = ctx.policy.eval(&argv, None);
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
        ExecDecision::Ask => ApprovalDecision::Ask {
            layer: ApprovalLayer::Layer2Starlark,
            reason: "layer-2 ask".into(),
        },
        ExecDecision::FallThrough => l1,
    };

    // 3. Layer 3 Mirror-learned — Phase 2; skipped here.

    match merged {
        ApprovalDecision::Auto { .. } => {
            emit_pair(&ctx, tool, payload, &merged, false).await;
            merged
        }
        ApprovalDecision::Ask { .. } => {
            emit_pair(&ctx, tool, payload, &merged, true).await;
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
) {
    let mut h = Sha256::new();
    h.update(payload.as_bytes());
    let args_hash = format!("{:x}", h.finalize());
    let req = AgentEvent::ApprovalRequested {
        request_id: ctx.request_id.clone(),
        tool: tool.into(),
        args_hash,
        layer: format!("{:?}", layer_of(decision)),
        rule_matched: rule_of(decision),
        mirror_history: None,
        sandbox_summary: String::new(),
        requires_user_input,
    };
    agent::execution::core::fan_out_event(ctx.event_tx, Some(ctx.domain_bus), req).await;
    if !requires_user_input {
        emit_resolved(ctx, decision).await;
    }
}

async fn emit_resolved<'a>(ctx: &GuardCtx<'a>, decision: &ApprovalDecision) {
    let res = AgentEvent::ApprovalResolved {
        request_id: ctx.request_id.clone(),
        decision: format!("{:?}", decision),
        decision_reason: reason_of(decision),
        latency_ms: 0,
        persisted_rule: None,
        decided_by: decided_by(decision).into(),
    };
    agent::execution::core::fan_out_event(ctx.event_tx, Some(ctx.domain_bus), res).await;
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
