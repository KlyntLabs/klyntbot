use klynt_core::approval::{ApprovalDecision, ApprovalLayer, LayerOutcomeAudit};

#[test]
fn ask_decision_carries_layer_audit() {
    let d = ApprovalDecision::ask_with_audit(
        ApprovalLayer::DefaultMode,
        "no rule matched",
        LayerOutcomeAudit {
            privacy_passed: true,
            layer1: "ask: no match".into(),
            layer2: "deferred: starlark fall-through".into(),
            layer3: "skipped: mirror disabled".into(),
        },
    );
    match d {
        ApprovalDecision::Ask {
            layer_audit: Some(a), ..
        } => {
            assert!(a.privacy_passed);
            assert!(a.layer3.contains("mirror disabled"));
        }
        _ => panic!("expected Ask with audit"),
    }
}

#[test]
fn ask_without_audit_has_none() {
    let d = ApprovalDecision::ask(ApprovalLayer::Layer1Declarative, "test");
    match d {
        ApprovalDecision::Ask {
            layer_audit: None, ..
        } => {}
        _ => panic!("expected Ask with None audit"),
    }
}
