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
            return ApprovalDecision::auto_deny(
                ApprovalLayer::Layer1Declarative,
                format!("layer-1 deny: {rule}"),
                Some(rule),
            );
        }
        if let Some(rule) = self.allow.find_match(tool, payload) {
            return ApprovalDecision::auto_allow(
                ApprovalLayer::Layer1Declarative,
                format!("layer-1 allow: {rule}"),
                Some(rule),
            );
        }
        if let Some(rule) = self.ask.find_match(tool, payload) {
            return ApprovalDecision::ask(
                ApprovalLayer::Layer1Declarative,
                format!("layer-1 ask: {rule}"),
            );
        }
        match self.default_if_no_match.as_str() {
            "allow" => ApprovalDecision::auto_allow(
                ApprovalLayer::Layer1Declarative,
                "layer-1 default: allow",
                None,
            ),
            "deny" => ApprovalDecision::auto_deny(
                ApprovalLayer::Layer1Declarative,
                "layer-1 default: deny",
                None,
            ),
            _ => ApprovalDecision::ask(ApprovalLayer::Layer1Declarative, "layer-1 default: ask"),
        }
    }
}
