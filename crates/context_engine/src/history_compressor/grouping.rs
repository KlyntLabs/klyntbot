use providers::Message;

use crate::token_counter::{self, TokenCounter};

use super::types::ConversationTurn;

/// Group a flat message list into conversation turns.
///
/// A turn starts at each `Message::User`. Everything until the next
/// `Message::User` belongs to the current turn. Leading system messages
/// form a preamble (returned separately). `ContextUpdate` messages
/// attach to their containing turn.
pub fn group_into_turns(
    messages: &[Message],
    token_counter: &dyn TokenCounter,
) -> (Vec<Message>, Vec<ConversationTurn>) {
    if messages.is_empty() {
        return (vec![], vec![]);
    }

    let mut preamble = Vec::new();
    let mut turns: Vec<ConversationTurn> = Vec::new();
    let mut current_turn_msgs: Vec<Message> = Vec::new();
    let mut seen_first_user = false;

    for msg in messages {
        match msg {
            Message::User { .. } => {
                if !seen_first_user {
                    seen_first_user = true;
                }
                // Start a new turn — flush the previous one
                if !current_turn_msgs.is_empty() {
                    let token_count = current_turn_msgs
                        .iter()
                        .map(|m| token_counter::estimate_message_tokens(token_counter, m))
                        .sum();
                    turns.push(ConversationTurn {
                        messages: std::mem::take(&mut current_turn_msgs),
                        turn_index: turns.len(),
                        token_count,
                        cognitive_score: None,
                        assigned_tier: None,
                    });
                }
                current_turn_msgs.push(msg.clone());
            }
            Message::System { .. } if !seen_first_user => {
                preamble.push(msg.clone());
            }
            _ => {
                if seen_first_user {
                    current_turn_msgs.push(msg.clone());
                } else {
                    // Orphan non-system messages before first user → preamble
                    preamble.push(msg.clone());
                }
            }
        }
    }

    // Flush the last turn
    if !current_turn_msgs.is_empty() {
        let token_count = current_turn_msgs
            .iter()
            .map(|m| token_counter::estimate_message_tokens(token_counter, m))
            .sum();
        turns.push(ConversationTurn {
            messages: std::mem::take(&mut current_turn_msgs),
            turn_index: turns.len(),
            token_count,
            cognitive_score: None,
            assigned_tier: None,
        });
    }

    (preamble, turns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_counter::default_token_counter;
    use std::sync::Arc;

    fn tc() -> Arc<dyn TokenCounter> {
        default_token_counter()
    }

    #[test]
    fn test_empty_history() {
        let (preamble, turns) = group_into_turns(&[], &*tc());
        assert!(preamble.is_empty());
        assert!(turns.is_empty());
    }

    #[test]
    fn test_system_preamble_extracted() {
        let msgs = vec![
            Message::system("You are a helpful assistant."),
            Message::system("Additional context."),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];
        let (preamble, turns) = group_into_turns(&msgs, &*tc());
        assert_eq!(preamble.len(), 2);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].messages.len(), 2); // user + assistant
        assert_eq!(turns[0].turn_index, 0);
    }

    #[test]
    fn test_multiple_turns() {
        let msgs = vec![
            Message::user("First question"),
            Message::assistant("First answer"),
            Message::user("Second question"),
            Message::assistant("Second answer"),
            Message::user("Third question"),
            Message::assistant("Third answer"),
        ];
        let (preamble, turns) = group_into_turns(&msgs, &*tc());
        assert!(preamble.is_empty());
        assert_eq!(turns.len(), 3);
        assert_eq!(turns[0].turn_index, 0);
        assert_eq!(turns[1].turn_index, 1);
        assert_eq!(turns[2].turn_index, 2);
    }

    #[test]
    fn test_tool_calls_stay_with_turn() {
        let msgs = vec![
            Message::user("Search for X"),
            Message::Assistant {
                content: None,
                tool_calls: Some(vec![providers::ToolCallMessage {
                    id: "tc1".into(),
                    r#type: "function".into(),
                    function: providers::FunctionCall {
                        name: "search".into(),
                        arguments: "{}".into(),
                    },
                }]),
                reasoning_content: None,
            },
            Message::Tool {
                tool_call_id: "tc1".into(),
                name: "search".into(),
                content: "Found 3 results".into(),
            },
            Message::assistant("Here are the results."),
        ];
        let (_, turns) = group_into_turns(&msgs, &*tc());
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].messages.len(), 4);
    }

    #[test]
    fn test_context_update_attaches_to_turn() {
        let msgs = vec![
            Message::user("Tell me about X"),
            Message::assistant("Let me look..."),
            Message::ContextUpdate {
                reason: "MemoryPromoted".into(),
                content: "New fact available".into(),
            },
            Message::user("What else?"),
            Message::assistant("Here's more."),
        ];
        let (_, turns) = group_into_turns(&msgs, &*tc());
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].messages.len(), 3); // user + assistant + context_update
        assert_eq!(turns[1].messages.len(), 2); // user + assistant
    }

    #[test]
    fn test_token_count_populated() {
        let msgs = vec![Message::user("Hello world"), Message::assistant("Hi!")];
        let (_, turns) = group_into_turns(&msgs, &*tc());
        assert!(turns[0].token_count > 0);
    }

    #[test]
    fn test_no_user_messages_returns_empty_turns() {
        let msgs = vec![
            Message::system("System prompt"),
            Message::assistant("Unprompted response"),
        ];
        let (preamble, turns) = group_into_turns(&msgs, &*tc());
        // System → preamble, assistant without user → no turn
        // The orphan assistant message has no user, so it gets attached to the preamble.
        assert!(turns.is_empty());
        assert_eq!(preamble.len(), 2); // system + orphan assistant
    }
}
