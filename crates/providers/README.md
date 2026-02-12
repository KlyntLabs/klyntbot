# klyntbot-providers

**LLM provider abstraction and implementations.**

## Overview

`klyntbot-providers` provides a unified interface for LLM providers:
- `LlmProvider` trait for provider implementations
- OpenAI-compatible HTTP client (no LiteLLM dependency)
- Provider registry with auto-detection
- Streaming response support
- Transcription provider for audio

Supports 12+ providers through a single HTTP interface.

## Contents

### LlmProvider Trait

```rust
use klyntbot_providers::{LlmProvider, Message, LlmResponse, ChatParams};
use async_trait::async_trait;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &[Message],
        params: &ChatParams,
    ) -> Result<LlmResponse>;

    async fn complete_streaming(
        &self,
        messages: &[Message],
        params: &ChatParams,
    ) -> Result<impl Stream<Item = Result<String>>>;
}
```

### Message Types

```rust
use klyntbot_providers::Message;
use klyntbot_core::MessageRole;

pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

pub struct ChatParams {
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub tools: Option<Vec<Tool>>,
}

pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: String,
}
```

### Creating a Provider

```rust
use klyntbot_providers::create_provider;
use klyntbot_config::Config;

let config = Config::default();
let provider = create_provider("claude-sonnet-4-5", &config)?;

let response = provider.complete(&messages, &params).await?;
println!("Response: {}", response.content);
```

### Provider Auto-Detection

The registry detects providers by model name or API key:

```rust
// Model name detection
"claude-sonnet-4-5" → Anthropic
"gpt-4o" → OpenAI
"deepseek-r1" → DeepSeek
"gemini-2.0" → Google

// API key prefix detection
"sk-ant-..." → Anthropic
"sk-or-..." → OpenRouter
```

### Supported Providers

| Provider | Models | Type |
|----------|--------|------|
| Anthropic | Claude 4.5/4.6 (Opus, Sonnet, Haiku) | Direct |
| OpenAI | GPT-4o, o1, o3 | Direct |
| DeepSeek | DeepSeek-R1, V3 | Direct |
| Google | Gemini 2.0, Gemini Pro | Direct |
| Groq | Llama 3.x, Mixtral, Whisper | Direct |
| OpenRouter | 200+ models from all providers | Gateway |
| AiHubMix | Multi-provider gateway | Gateway |
| Zhipu | GLM-4, GLM-Z | Direct |
| DashScope | Qwen models | Direct |
| Moonshot | Kimi K2.5 | Direct |
| MiniMax | MiniMax models | Direct |
| vLLM | Any local model | Local |

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
klyntbot-providers.workspace = true
```

Example:

```rust
use klyntbot_providers::{create_provider, Message, ChatParams};
use klyntbot_config::Config;
use klyntbot_core::MessageRole;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    let provider = create_provider("claude-sonnet-4-5", &config)?;

    let messages = vec![
        Message {
            role: MessageRole::User,
            content: "What is Rust?".into(),
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let params = ChatParams {
        model: "claude-sonnet-4-5-20250929".into(),
        max_tokens: Some(1024),
        temperature: Some(0.7),
        tools: None,
    };

    let response = provider.complete(&messages, &params).await?;
    println!("{}", response.content);

    Ok(())
}
```

## OpenAI-Compatible Implementation

All providers use the same HTTP endpoint format:

```
POST /v1/chat/completions
Content-Type: application/json

{
  "model": "...",
  "messages": [...],
  "max_tokens": 1024,
  "temperature": 0.7
}
```

**No LiteLLM dependency** — klyntbot implements this directly in ~400 lines.

### Custom Base URLs

```rust
let mut config = Config::default();
config.providers.vllm = Some(ProviderApiConfig {
    api_key: Secret::new("dummy".into()),
    api_base: Some("http://localhost:8000/v1".into()),
    extra_headers: None,
});
```

## Streaming Responses

```rust
use futures_util::StreamExt;

let mut stream = provider.complete_streaming(&messages, &params).await?;

while let Some(chunk) = stream.next().await {
    match chunk {
        Ok(text) => print!("{}", text),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Transcription Provider

For audio transcription (e.g., Telegram voice messages):

```rust
use klyntbot_providers::TranscriptionProvider;

let transcriber = TranscriptionProvider::new(&config)?;
let text = transcriber.transcribe(audio_bytes, "whisper-large-v3").await?;
```

## Design Principles

1. **Unified interface** — All providers implement `LlmProvider` trait
2. **OpenAI-compatible** — Standard `/v1/chat/completions` endpoint
3. **Auto-detection** — Model name → provider mapping
4. **Streaming support** — Async streams for real-time responses
5. **No LiteLLM** — Direct HTTP client for minimal dependencies

## Dependencies

- `klyntbot-core` — Error types, shared types
- `klyntbot-config` — Configuration loading
- `async-trait` — Async trait support
- `reqwest` — HTTP client
- `serde`, `serde_json` — Serialization
- `tokio`, `futures-util` — Async runtime and streams

## See Also

- [klyntbot Architecture](../../docs/ARCHITECTURE.md)
- [LLM Providers](../../README.md#llm-providers)
