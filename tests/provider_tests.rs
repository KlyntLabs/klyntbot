//! Integration tests for LLM providers

use klyntbot::providers::registry::ProviderRegistry;
use klyntbot::providers::types::*;

/// Test provider registry model resolution
#[test]
fn test_provider_registry_model_resolution() {
    // Test Anthropic model detection
    let claude_spec = ProviderRegistry::find_by_model("claude-opus-4");
    assert!(claude_spec.is_some());
    assert_eq!(claude_spec.unwrap().name, "anthropic");

    let claude_spec2 = ProviderRegistry::find_by_model("claude-sonnet-3.5");
    assert!(claude_spec2.is_some());
    assert_eq!(claude_spec2.unwrap().name, "anthropic");

    // Test OpenAI model detection
    let gpt_spec = ProviderRegistry::find_by_model("gpt-4");
    assert!(gpt_spec.is_some());
    assert_eq!(gpt_spec.unwrap().name, "openai");

    let gpt_spec2 = ProviderRegistry::find_by_model("gpt-3.5-turbo");
    assert!(gpt_spec2.is_some());
    assert_eq!(gpt_spec2.unwrap().name, "openai");

    // Test DeepSeek model detection
    let deepseek_spec = ProviderRegistry::find_by_model("deepseek-chat");
    assert!(deepseek_spec.is_some());
    assert_eq!(deepseek_spec.unwrap().name, "deepseek");

    // Test Gemini model detection
    let gemini_spec = ProviderRegistry::find_by_model("gemini-pro");
    assert!(gemini_spec.is_some());
    assert_eq!(gemini_spec.unwrap().name, "gemini");

    // Test unknown model
    let unknown_spec = ProviderRegistry::find_by_model("unknown-model-xyz");
    assert!(unknown_spec.is_none());
}

/// Test provider registry by name lookup
#[test]
fn test_provider_registry_by_name() {
    let anthropic = ProviderRegistry::find_by_name("anthropic");
    assert!(anthropic.is_some());
    assert!(anthropic.unwrap().label().contains("Anthropic"));

    let openai = ProviderRegistry::find_by_name("openai");
    assert!(openai.is_some());
    assert!(openai.unwrap().label().contains("OpenAI"));

    let deepseek = ProviderRegistry::find_by_name("deepseek");
    assert!(deepseek.is_some());
    assert!(deepseek.unwrap().label().contains("DeepSeek"));

    let nonexistent = ProviderRegistry::find_by_name("nonexistent");
    assert!(nonexistent.is_none());
}

/// Test gateway detection (OpenRouter, AiHubMix)
#[test]
fn test_provider_registry_gateway_detection() {
    // Test OpenRouter key prefix detection
    let openrouter_key = "sk-or-v1-xxxxxxxxxxxxx";
    let gateway = ProviderRegistry::find_gateway(None, Some(openrouter_key), None);
    assert!(gateway.is_some());
    assert_eq!(gateway.unwrap().name, "openrouter");

    // Test AiHubMix base URL detection
    let gateway2 =
        ProviderRegistry::find_gateway(None, Some("any-key"), Some("https://aihubmix.com/v1"));
    assert!(gateway2.is_some());
    // AiHubMix might be registered as its own provider or as openrouter-compatible
    let name = gateway2.unwrap().name;
    assert!(name == "aihubmix" || name == "openrouter");

    // Test explicit name matching
    let explicit = ProviderRegistry::find_gateway(Some("openrouter"), Some("any-key"), None);
    assert!(explicit.is_some());
    assert_eq!(explicit.unwrap().name, "openrouter");
}

/// Test message construction helpers
#[test]
fn test_message_construction() {
    // Test system message
    let sys_msg = Message::system("You are a helpful assistant");
    match sys_msg {
        Message::System { content } => {
            assert_eq!(content, "You are a helpful assistant");
        }
        _ => panic!("Expected System message"),
    }

    // Test user message
    let user_msg = Message::user("Hello!");
    match user_msg {
        Message::User { content } => match content {
            UserContent::Text(text) => assert_eq!(text, "Hello!"),
            _ => panic!("Expected Text content"),
        },
        _ => panic!("Expected User message"),
    }

    // Test assistant message
    let asst_msg = Message::assistant("Hi there!");
    match asst_msg {
        Message::Assistant {
            content,
            tool_calls,
            reasoning_content,
        } => {
            assert_eq!(content, Some("Hi there!".to_string()));
            assert!(tool_calls.is_none());
            assert!(reasoning_content.is_none());
        }
        _ => panic!("Expected Assistant message"),
    }

    // Test tool result message
    let tool_msg = Message::tool("call_123", "read_file", "File contents");
    match tool_msg {
        Message::Tool {
            tool_call_id,
            name,
            content,
        } => {
            assert_eq!(tool_call_id, "call_123");
            assert_eq!(name, "read_file");
            assert_eq!(content, "File contents");
        }
        _ => panic!("Expected Tool message"),
    }
}

/// Test multipart message with image
#[test]
fn test_multipart_message_with_image() {
    let parts = vec![
        ContentPart::Text {
            text: "What's in this image?".to_string(),
        },
        ContentPart::ImageUrl {
            image_url: ImageUrl {
                url: "data:image/png;base64,iVBORw0KGgoAAAANS...".to_string(),
            },
        },
    ];

    let msg = Message::user_multipart(parts.clone());
    match msg {
        Message::User { content } => match content {
            UserContent::MultiPart(parts_out) => {
                assert_eq!(parts_out.len(), 2);
                match &parts_out[0] {
                    ContentPart::Text { text } => assert_eq!(text, "What's in this image?"),
                    _ => panic!("Expected Text part"),
                }
                match &parts_out[1] {
                    ContentPart::ImageUrl { image_url } => {
                        assert!(image_url.url.starts_with("data:image/png"));
                    }
                    _ => panic!("Expected ImageUrl part"),
                }
            }
            _ => panic!("Expected MultiPart content"),
        },
        _ => panic!("Expected User message"),
    }
}

/// Test ToolCall serialization and deserialization
#[test]
fn test_tool_call_serialization() {
    let tool_call = ToolCall {
        id: "call_abc123".to_string(),
        name: "read_file".to_string(),
        arguments: serde_json::json!({
            "path": "/path/to/file.txt"
        }),
    };

    // Serialize to JSON
    let json = serde_json::to_string(&tool_call).unwrap();
    assert!(json.contains("call_abc123"));
    assert!(json.contains("read_file"));
    assert!(json.contains("/path/to/file.txt"));

    // Deserialize back
    let deserialized: ToolCall = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "call_abc123");
    assert_eq!(deserialized.name, "read_file");
    assert_eq!(
        deserialized.arguments["path"].as_str().unwrap(),
        "/path/to/file.txt"
    );
}

/// Test Usage struct
#[test]
fn test_usage_struct() {
    let usage = Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };

    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 150);

    // Test default
    let default_usage = Usage::default();
    assert_eq!(default_usage.prompt_tokens, 0);
    assert_eq!(default_usage.completion_tokens, 0);
    assert_eq!(default_usage.total_tokens, 0);
}

/// Test LlmResponse serialization
#[test]
fn test_llm_response_serialization() {
    let response = LlmResponse {
        content: Some("Hello! How can I help?".to_string()),
        tool_calls: vec![],
        finish_reason: "stop".to_string(),
        usage: Usage {
            prompt_tokens: 50,
            completion_tokens: 20,
            total_tokens: 70,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        },
        reasoning_content: None,
    };

    // Serialize to JSON
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("Hello! How can I help?"));
    assert!(json.contains("stop"));

    // Deserialize back
    let deserialized: LlmResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(
        deserialized.content,
        Some("Hello! How can I help?".to_string())
    );
    assert_eq!(deserialized.finish_reason, "stop");
    assert_eq!(deserialized.usage.total_tokens, 70);
}

/// Test LlmResponse with tool calls
#[test]
fn test_llm_response_with_tool_calls() {
    let response = LlmResponse {
        content: None,
        tool_calls: vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": "file1.txt"}),
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "write_file".to_string(),
                arguments: serde_json::json!({"path": "file2.txt", "content": "data"}),
            },
        ],
        finish_reason: "tool_calls".to_string(),
        usage: Usage::default(),
        reasoning_content: None,
    };

    assert!(response.content.is_none());
    assert_eq!(response.tool_calls.len(), 2);
    assert_eq!(response.tool_calls[0].name, "read_file");
    assert_eq!(response.tool_calls[1].name, "write_file");
    assert_eq!(response.finish_reason, "tool_calls");
}

/// Test reasoning content (for thinking models like DeepSeek-R1)
#[test]
fn test_llm_response_with_reasoning() {
    let response = LlmResponse {
        content: Some("The answer is 42".to_string()),
        tool_calls: vec![],
        finish_reason: "stop".to_string(),
        usage: Usage::default(),
        reasoning_content: Some(
            "Let me think about this... The question of life, the universe, and everything..."
                .to_string(),
        ),
    };

    assert!(response.reasoning_content.is_some());
    assert!(response
        .reasoning_content
        .as_ref()
        .unwrap()
        .contains("Let me think"));
}
