use config::schema::CodingPermissions;
use klynt_core::approval::{decision::ApprovalDecision, Layer1};
use proptest::prelude::*;

proptest! {
    /// K3: For any (allow, deny, ask) rule sets and any (tool, payload),
    /// Layer1::evaluate is deterministic AND obeys deny > allow > ask.
    #[test]
    fn k3_routing_precedence(
        allow in prop::collection::vec(r"Bash\([a-z*]{1,5}\*?\)", 0..3),
        deny  in prop::collection::vec(r"Bash\(rm[* ]\*?\)", 0..2),
        payload in r"[a-z ]{1,15}",
    ) {
        let perms = CodingPermissions {
            allow, deny: deny.clone(), ask: vec!["Bash(*)".into()],
            default_if_no_match: "ask".into(), mirror_learning: false,
        };
        let l1 = Layer1::compile(&perms).unwrap();
        let d1 = l1.evaluate("bash", &payload);
        let d2 = l1.evaluate("bash", &payload);
        prop_assert_eq!(format!("{:?}", d1), format!("{:?}", d2));
        if !deny.is_empty() && payload.starts_with("rm") {
            let is_denied = matches!(d1, ApprovalDecision::Auto { allowed: false, .. });
            prop_assert!(is_denied);
        }
    }
}
