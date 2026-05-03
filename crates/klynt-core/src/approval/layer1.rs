use super::{
    decision::{ApprovalDecision, ApprovalLayer},
    matcher::{CompiledRules, MatcherError},
};
use config::schema::CodingPermissions;

pub struct Layer1 {
    allow: CompiledRules,
    deny: CompiledRules,
    ask: CompiledRules,
    default_if_no_match: String,
}

impl Layer1 {
    pub fn compile(p: &CodingPermissions) -> Result<Self, MatcherError> {
        Ok(Self {
            allow: CompiledRules::compile(&p.allow)?,
            deny: CompiledRules::compile(&p.deny)?,
            ask: CompiledRules::compile(&p.ask)?,
            default_if_no_match: p.default_if_no_match.clone(),
        })
    }
    pub fn evaluate(&self, tool: &str, payload: &str) -> ApprovalDecision {
        if let Some(rule) = self.deny.find_match(tool, payload) {
            return ApprovalDecision::Auto {
                allowed: false,
                layer: ApprovalLayer::Layer1Declarative,
                reason: format!("layer-1 deny: {rule}"),
                rule_matched: Some(rule),
            };
        }
        if let Some(rule) = self.allow.find_match(tool, payload) {
            return ApprovalDecision::Auto {
                allowed: true,
                layer: ApprovalLayer::Layer1Declarative,
                reason: format!("layer-1 allow: {rule}"),
                rule_matched: Some(rule),
            };
        }
        if let Some(rule) = self.ask.find_match(tool, payload) {
            return ApprovalDecision::ask(
                ApprovalLayer::Layer1Declarative,
                format!("layer-1 ask: {rule}"),
            );
        }
        match self.default_if_no_match.as_str() {
            "allow" => ApprovalDecision::Auto {
                allowed: true,
                layer: ApprovalLayer::Layer1Declarative,
                reason: "layer-1 default: allow".into(),
                rule_matched: None,
            },
            "deny" => ApprovalDecision::Auto {
                allowed: false,
                layer: ApprovalLayer::Layer1Declarative,
                reason: "layer-1 default: deny".into(),
                rule_matched: None,
            },
            _ => ApprovalDecision::ask(
                ApprovalLayer::Layer1Declarative,
                "layer-1 default: ask",
            ),
        }
    }
}
