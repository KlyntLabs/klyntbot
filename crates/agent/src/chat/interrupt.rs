//! Interrupt handling for concurrent user messages during agent processing.
//!
//! When a user sends a new message while the agent is still processing a previous
//! one, the `InterruptHandler` decides what to do based on the configured policy.

/// Policy for handling interrupts when a new message arrives during processing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum InterruptPolicy {
    /// Queue the new message — process it after the current task finishes.
    #[default]
    Queue,
    /// Cancel the current task and switch to the new message.
    CancelAndSwitch,
    /// Merge the new message into the current processing context.
    Merge,
}

/// Handles interrupts when new messages arrive during agent processing.
pub struct InterruptHandler {
    policy: InterruptPolicy,
    /// Minimum processing duration (ms) before an interrupt is considered.
    /// Short tasks shouldn't be interrupted.
    min_interrupt_duration_ms: u64,
}

impl InterruptHandler {
    pub fn new(policy: InterruptPolicy) -> Self {
        Self {
            policy,
            min_interrupt_duration_ms: 2000,
        }
    }

    pub fn with_min_duration(mut self, ms: u64) -> Self {
        self.min_interrupt_duration_ms = ms;
        self
    }

    /// Determine if the current processing should be interrupted.
    ///
    /// Returns `true` if the policy suggests interrupting. Queue policy never
    /// interrupts. CancelAndSwitch interrupts after the minimum duration.
    /// Merge always "interrupts" (to inject the new context).
    pub fn should_interrupt(&self, current_duration_ms: u64, _new_message: &str) -> bool {
        match self.policy {
            InterruptPolicy::Queue => false,
            InterruptPolicy::CancelAndSwitch => {
                current_duration_ms >= self.min_interrupt_duration_ms
            }
            InterruptPolicy::Merge => current_duration_ms >= self.min_interrupt_duration_ms,
        }
    }

    /// Handle the interrupt by producing the message to process next.
    ///
    /// - Queue: returns the new message (to be queued)
    /// - CancelAndSwitch: returns the new message (current task is cancelled)
    /// - Merge: combines pending messages with the new one
    pub fn handle(&self, pending: &[String], new_message: &str) -> String {
        match self.policy {
            InterruptPolicy::Queue | InterruptPolicy::CancelAndSwitch => {
                new_message.to_string()
            }
            InterruptPolicy::Merge => {
                if pending.is_empty() {
                    return new_message.to_string();
                }
                let mut merged = String::new();
                for msg in pending {
                    merged.push_str(msg);
                    merged.push('\n');
                }
                merged.push_str(new_message);
                merged
            }
        }
    }

    pub fn policy(&self) -> &InterruptPolicy {
        &self.policy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_policy_never_interrupts() {
        let handler = InterruptHandler::new(InterruptPolicy::Queue);
        assert!(!handler.should_interrupt(0, "new msg"));
        assert!(!handler.should_interrupt(5000, "new msg"));
        assert!(!handler.should_interrupt(100_000, "new msg"));
    }

    #[test]
    fn test_cancel_policy_respects_min_duration() {
        let handler = InterruptHandler::new(InterruptPolicy::CancelAndSwitch);
        assert!(!handler.should_interrupt(500, "urgent"));
        assert!(!handler.should_interrupt(1999, "urgent"));
        assert!(handler.should_interrupt(2000, "urgent"));
        assert!(handler.should_interrupt(5000, "urgent"));
    }

    #[test]
    fn test_merge_policy_respects_min_duration() {
        let handler = InterruptHandler::new(InterruptPolicy::Merge);
        assert!(!handler.should_interrupt(1000, "add this"));
        assert!(handler.should_interrupt(3000, "add this"));
    }

    #[test]
    fn test_handle_queue_returns_new_message() {
        let handler = InterruptHandler::new(InterruptPolicy::Queue);
        let result = handler.handle(&["old msg".to_string()], "new msg");
        assert_eq!(result, "new msg");
    }

    #[test]
    fn test_handle_cancel_returns_new_message() {
        let handler = InterruptHandler::new(InterruptPolicy::CancelAndSwitch);
        let result = handler.handle(&["old msg".to_string()], "new msg");
        assert_eq!(result, "new msg");
    }

    #[test]
    fn test_handle_merge_combines_messages() {
        let handler = InterruptHandler::new(InterruptPolicy::Merge);
        let pending = vec![
            "first message".to_string(),
            "second message".to_string(),
        ];
        let result = handler.handle(&pending, "third message");
        assert!(result.contains("first message"));
        assert!(result.contains("second message"));
        assert!(result.contains("third message"));
    }

    #[test]
    fn test_handle_merge_empty_pending() {
        let handler = InterruptHandler::new(InterruptPolicy::Merge);
        let result = handler.handle(&[], "only message");
        assert_eq!(result, "only message");
    }

    #[test]
    fn test_custom_min_duration() {
        let handler =
            InterruptHandler::new(InterruptPolicy::CancelAndSwitch).with_min_duration(500);
        assert!(!handler.should_interrupt(499, "msg"));
        assert!(handler.should_interrupt(500, "msg"));
    }
}
