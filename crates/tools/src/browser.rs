//! Browser automation tool using the agent-browser CLI.

// ── Write-action detection ────────────────────────────────────────────────────

/// Returns `true` if this action + element label combination is a write action
/// that should be guarded in Autonomous (or Strict) mode.
pub fn is_write_action(action: &str, element_label: &str) -> bool {
    // submit_and_confirm is always a write action
    if action == "submit_and_confirm" {
        return true;
    }

    let label = element_label.to_lowercase();

    // Dangerous click targets
    if action == "click" {
        let dangerous = [
            "submit", "checkout", "buy", "purchase", "confirm",
            "place order", "delete", "remove", "send", "pay",
        ];
        if dangerous.iter().any(|k| label.contains(k)) {
            return true;
        }
    }

    // Payment field fills
    if action == "fill" || action == "type" {
        let payment = ["card number", "cvv", "cvc", "expiry", "billing"];
        if payment.iter().any(|k| label.contains(k)) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::TrustLevel;

    // ── TrustLevel (imported from config) ─────────────────────────────────────

    #[test]
    fn test_trust_level_autonomous_is_default() {
        assert_eq!(TrustLevel::default(), TrustLevel::Autonomous);
    }

    // ── Write-action guard ────────────────────────────────────────────────────

    #[test]
    fn test_submit_and_confirm_always_write() {
        assert!(is_write_action("submit_and_confirm", ""));
        assert!(is_write_action("submit_and_confirm", "Anything"));
    }

    #[test]
    fn test_click_dangerous_labels() {
        assert!(is_write_action("click", "Place Order"));
        assert!(is_write_action("click", "Checkout Now"));
        assert!(is_write_action("click", "Buy Now"));
        assert!(is_write_action("click", "Confirm Purchase"));
        assert!(is_write_action("click", "Delete Account"));
        assert!(is_write_action("click", "Send Message"));
        assert!(is_write_action("click", "Pay $49.99"));
    }

    #[test]
    fn test_click_safe_labels_not_write() {
        assert!(!is_write_action("click", "Search"));
        assert!(!is_write_action("click", "Next"));
        assert!(!is_write_action("click", "View Cart"));
        assert!(!is_write_action("click", "Add to Cart"));
        assert!(!is_write_action("click", "Learn More"));
    }

    #[test]
    fn test_fill_payment_fields_write() {
        assert!(is_write_action("fill", "Card Number"));
        assert!(is_write_action("fill", "CVV"));
        assert!(is_write_action("fill", "Expiry Date"));
        assert!(is_write_action("fill", "Billing Address"));
    }

    #[test]
    fn test_fill_regular_fields_not_write() {
        assert!(!is_write_action("fill", "Email"));
        assert!(!is_write_action("fill", "Username"));
        assert!(!is_write_action("fill", "Search"));
        assert!(!is_write_action("fill", "City"));
    }

    #[test]
    fn test_navigate_snapshot_never_write() {
        assert!(!is_write_action("navigate", ""));
        assert!(!is_write_action("snapshot", ""));
        assert!(!is_write_action("screenshot", ""));
        assert!(!is_write_action("get_text", ""));
    }
}
