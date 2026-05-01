use super::{
    decision::{ApprovalDecision, ApprovalLayer},
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

    // 3. Layer 3 — Mirror-learned (opt-in)
    if let Some(repo) = ctx.history_repo.as_ref() {
        let cfg = crate::approval::layer3::Layer3Config {
            enabled: ctx.mirror_learning_enabled,
            min_approvals: ctx.mirror_min_approvals,
            cooldown_seconds: ctx.mirror_cooldown_seconds,
        };
        let args_json = ctx.args.as_ref().map(|v| v.to_string()).unwrap_or_default();
        let hash = crate::approval::layer3::args_hash_for_relevance(tool, &args_json);
        let summary = repo.summary(tool, &hash, &ctx.repo_id).await
            .unwrap_or_default();
        match crate::approval::layer3::evaluate(&cfg, &summary, ctx.now_unix) {
            crate::approval::layer3::Layer3Outcome::AutoAllow { reason } => {
                let decision = ApprovalDecision::auto_allow(ApprovalLayer::Layer3Mirror, reason);
                emit_pair(&ctx, tool, payload, &decision, false).await;
                return decision;
            }
            crate::approval::layer3::Layer3Outcome::Ask { reason } => {
                let decision = ApprovalDecision::ask(ApprovalLayer::Layer3Mirror, reason);
                emit_pair(&ctx, tool, payload, &decision, true).await;
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
            crate::approval::layer3::Layer3Outcome::FallThrough => { /* continue */ }
        }
    }

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
    let req = ToolEvent::ApprovalRequested {
        request_id: ctx.request_id.clone(),
        tool: tool.into(),
        args_hash,
        layer: format!("{:?}", layer_of(decision)),
        rule_matched: rule_of(decision),
        mirror_history: None,
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
