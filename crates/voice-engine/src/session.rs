//! Voice capture session state machine.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceSessionState {
    Idle,
    Capturing,
    Finalizing,
    WaitingForResponse,
    Complete,
}

impl VoiceSessionState {
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Idle | Self::Complete)
    }

    pub fn can_transition_to(&self, next: VoiceSessionState) -> bool {
        matches!(
            (self, next),
            (Self::Idle, Self::Capturing)
                | (Self::Capturing, Self::Finalizing)
                | (Self::Capturing, Self::Idle)
                | (Self::Finalizing, Self::WaitingForResponse)
                | (Self::Finalizing, Self::Idle)
                | (Self::WaitingForResponse, Self::Complete)
                | (Self::WaitingForResponse, Self::Idle)
                | (Self::Complete, Self::Idle)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_can_start_capturing() {
        assert!(VoiceSessionState::Idle.can_transition_to(VoiceSessionState::Capturing));
    }

    #[test]
    fn capturing_can_finalize_or_cancel() {
        assert!(VoiceSessionState::Capturing.can_transition_to(VoiceSessionState::Finalizing));
        assert!(VoiceSessionState::Capturing.can_transition_to(VoiceSessionState::Idle));
    }

    #[test]
    fn finalizing_can_proceed_or_cancel() {
        assert!(VoiceSessionState::Finalizing.can_transition_to(VoiceSessionState::WaitingForResponse));
        assert!(VoiceSessionState::Finalizing.can_transition_to(VoiceSessionState::Idle));
    }

    #[test]
    fn complete_returns_to_idle() {
        assert!(VoiceSessionState::Complete.can_transition_to(VoiceSessionState::Idle));
        assert!(!VoiceSessionState::Complete.can_transition_to(VoiceSessionState::Capturing));
    }

    #[test]
    fn invalid_transitions_rejected() {
        assert!(!VoiceSessionState::Idle.can_transition_to(VoiceSessionState::Finalizing));
        assert!(!VoiceSessionState::Capturing.can_transition_to(VoiceSessionState::Complete));
    }

    #[test]
    fn active_states() {
        assert!(!VoiceSessionState::Idle.is_active());
        assert!(VoiceSessionState::Capturing.is_active());
        assert!(VoiceSessionState::Finalizing.is_active());
        assert!(VoiceSessionState::WaitingForResponse.is_active());
        assert!(!VoiceSessionState::Complete.is_active());
    }
}
