# Architecture Design: 3 Complex Features

## Feature 1: Streaming LLM Responses

### Current State Analysis
- `OpenAiCompatProvider::chat()` uses synchronous reqwest POST with `.json()` which waits for complete response
- `LlmProvider` trait has only `async fn chat()` returning `Result<LlmResponse>`
- Agent loop calls `provider.chat()` and blocks until full response arrives
- No incremental output during long LLM generations

### Design: Add Streaming Support

#### 1. Extend LlmProvider Trait

Add an optional streaming method with default fallback:

```rust
// src/providers/types.rs

use futures_util::Stream;
use std::pin::Pin;

/// Streaming chunk from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStreamChunk {
    /// Incremental content (text delta)
    pub content: Option<String>,

    /// Tool call delta (accumulated across chunks)
    pub tool_call_delta: Option<ToolCallDelta>,

    /// True if this is the final chunk
    pub is_final: bool,

    /// Finish reason (only present in final chunk)
    pub finish_reason: Option<String>,

    /// Reasoning content delta (for thinking models)
    pub reasoning_content: Option<String>,
}

/// Tool call delta for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
}

/// Stream type alias
pub type LlmStream = Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a chat completion request (non-streaming)
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        model: Option<&str>,
    ) -> Result<LlmResponse>;

    /// Send a streaming chat completion request
    /// Default implementation falls back to non-streaming chat()
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        model: Option<&str>,
    ) -> Result<LlmStream> {
        // Default: call chat() and emit a single chunk
        let response = self.chat(messages, tools, model).await?;

        let chunk = LlmStreamChunk {
            content: response.content,
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some(response.finish_reason),
            reasoning_content: response.reasoning_content,
        };

        Ok(Box::pin(futures_util::stream::once(async move {
            Ok(chunk)
        })))
    }

    /// Check if streaming is supported
    fn supports_streaming(&self) -> bool {
        false
    }

    fn default_model(&self) -> &str;
    fn name(&self) -> &str;
}
```

#### 2. Implement Streaming in OpenAiCompatProvider

```rust
// src/providers/openai_compat.rs

use futures_util::{Stream, StreamExt};
use std::pin::Pin;

impl OpenAiCompatProvider {
    /// Parse SSE event data
    fn parse_sse_chunk(&self, data: &str) -> Result<Option<LlmStreamChunk>> {
        // Handle [DONE] marker
        if data.trim() == "[DONE]" {
            return Ok(None);
        }

        // Parse JSON
        let value: Value = serde_json::from_str(data)
            .map_err(|e| ProviderError::InvalidResponse(format!("Invalid SSE JSON: {}", e)))?;

        let choices = value["choices"].as_array()
            .ok_or_else(|| ProviderError::InvalidResponse("No choices in SSE chunk".to_string()))?;

        if choices.is_empty() {
            return Ok(None);
        }

        let choice = &choices[0];
        let delta = &choice["delta"];

        // Extract content delta
        let content = delta["content"].as_str().map(|s| s.to_string());

        // Extract reasoning content delta
        let reasoning_content = delta["reasoning_content"].as_str().map(|s| s.to_string());

        // Extract tool call delta
        let tool_call_delta = if let Some(tool_calls) = delta["tool_calls"].as_array() {
            if !tool_calls.is_empty() {
                let tc = &tool_calls[0];
                Some(ToolCallDelta {
                    index: tc["index"].as_u64().unwrap_or(0) as usize,
                    id: tc["id"].as_str().map(|s| s.to_string()),
                    name: tc["function"]["name"].as_str().map(|s| s.to_string()),
                    arguments: tc["function"]["arguments"].as_str().map(|s| s.to_string()),
                })
            } else {
                None
            }
        } else {
            None
        };

        // Check if final
        let finish_reason = choice["finish_reason"].as_str().map(|s| s.to_string());
        let is_final = finish_reason.is_some();

        Ok(Some(LlmStreamChunk {
            content,
            tool_call_delta,
            is_final,
            finish_reason,
            reasoning_content,
        }))
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    // ... existing chat() method ...

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        model: Option<&str>,
    ) -> Result<LlmStream> {
        let url = format!("{}/chat/completions", self.api_base);
        let model = model.unwrap_or(&self.default_model);

        // Build request body with stream: true
        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": true,
        });

        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = json!(tools);
                body["tool_choice"] = json!("auto");
            }
        }

        debug!("Calling LLM (streaming): model={}, messages={}", model, messages.len());

        // Build request
        let mut request = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        for (key, value) in &self.extra_headers {
            request = request.header(key, value);
        }

        // Send request and get response stream
        let response = request
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(KlyntbotError::Provider(ProviderError::InvalidResponse(
                format!("HTTP {}: {}", status, error_text),
            )));
        }

        // Create SSE stream
        let stream = response.bytes_stream();

        // Buffer for incomplete lines
        let mut line_buffer = String::new();

        let chunk_stream = stream.map(move |result| {
            match result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    line_buffer.push_str(&text);

                    // Process complete lines
                    let mut chunks = Vec::new();
                    while let Some(newline_pos) = line_buffer.find('\n') {
                        let line = line_buffer[..newline_pos].trim();
                        line_buffer = line_buffer[newline_pos + 1..].to_string();

                        // Parse SSE format: "data: {...}"
                        if let Some(data) = line.strip_prefix("data: ") {
                            if let Ok(Some(chunk)) = self.parse_sse_chunk(data) {
                                chunks.push(chunk);
                            }
                        }
                    }

                    Ok(futures_util::stream::iter(chunks.into_iter().map(Ok)))
                }
                Err(e) => Err(KlyntbotError::Provider(ProviderError::Http(e))),
            }
        })
        .flat_map(|result| {
            match result {
                Ok(stream) => futures_util::stream::Either::Left(stream),
                Err(e) => futures_util::stream::Either::Right(futures_util::stream::once(async move { Err(e) })),
            }
        });

        Ok(Box::pin(chunk_stream))
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    // ... existing default_model() and name() methods ...
}
```

#### 3. Update Agent Loop to Use Streaming

```rust
// src/agent/agent_loop.rs

impl AgentLoop {
    /// Process message with streaming (for real-time output)
    async fn process_message_streaming(&self, msg: InboundMessage) -> Result<()> {
        // ... setup code (same as current process_message) ...

        for iteration in 0..max_iterations {
            debug!("Agent iteration {}/{}", iteration + 1, max_iterations);

            let tool_registry = self.tool_registry.read().await;
            let tools = tool_registry.get_definitions();
            drop(tool_registry);

            // Check if provider supports streaming
            if self.provider.supports_streaming() {
                // Use streaming
                let mut stream = self
                    .provider
                    .chat_stream(
                        &current_messages,
                        Some(&tools),
                        Some(&self.config.agents.defaults.model),
                    )
                    .await?;

                // Accumulate response
                let mut accumulated_content = String::new();
                let mut accumulated_tool_calls: HashMap<usize, ToolCallAccumulator> = HashMap::new();
                let mut finish_reason = None;

                // Process stream chunks
                while let Some(chunk_result) = stream.next().await {
                    let chunk = chunk_result?;

                    // Accumulate content
                    if let Some(content) = chunk.content {
                        accumulated_content.push_str(&content);
                        // TODO: Stream to channel in real-time (for CLI, print immediately)
                    }

                    // Accumulate tool calls
                    if let Some(delta) = chunk.tool_call_delta {
                        let accumulator = accumulated_tool_calls
                            .entry(delta.index)
                            .or_insert_with(|| ToolCallAccumulator::new());

                        if let Some(id) = delta.id {
                            accumulator.id = id;
                        }
                        if let Some(name) = delta.name {
                            accumulator.name = name;
                        }
                        if let Some(args) = delta.arguments {
                            accumulator.arguments.push_str(&args);
                        }
                    }

                    // Check if final
                    if chunk.is_final {
                        finish_reason = chunk.finish_reason;
                        break;
                    }
                }

                // Build tool calls from accumulated data
                let tool_calls: Vec<ToolCall> = accumulated_tool_calls
                    .into_iter()
                    .map(|(_, acc)| {
                        let arguments: Value = serde_json::from_str(&acc.arguments)
                            .unwrap_or_else(|_| json!({"raw": acc.arguments}));

                        ToolCall {
                            id: acc.id,
                            name: acc.name,
                            arguments,
                        }
                    })
                    .collect();

                // Handle tool calls or final content (same logic as before)
                if !tool_calls.is_empty() {
                    // Execute tools...
                    continue;
                }

                if !accumulated_content.is_empty() {
                    final_content = Some(accumulated_content);
                    break;
                }
            } else {
                // Fall back to non-streaming (existing code)
                let response = self.provider.chat(&current_messages, Some(&tools), Some(&self.config.agents.defaults.model)).await?;
                // ... existing logic ...
            }
        }

        // ... rest of method (save session, send response) ...
    }
}

/// Helper to accumulate tool call data across chunks
struct ToolCallAccumulator {
    id: String,
    name: String,
    arguments: String,
}

impl ToolCallAccumulator {
    fn new() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            arguments: String::new(),
        }
    }
}
```

#### 4. Required Dependencies

Add to Cargo.toml:

```toml
futures-util = "0.3.31"  # Already present
```

---

## Feature 2: Email Channel Fix

### Current State Analysis
- Email channel implementation exists in `src/channels/email.rs`
- Uses `async-imap` v0.11.2 with `runtime-tokio` feature
- Commented out in `manager.rs` with note: "async-imap API issues"
- Code looks correct but there may be an API mismatch

### Root Cause Analysis

The issue is likely related to the `async-imap` v0.11.x API changes:

1. **Connection method returns `Result<Client, _>` directly** - no intermediate `connect()` step
2. **`secure()` method signature changed** - now requires both connector and domain
3. **`login()` returns `Result<Session, (Error, Client)>`** - special error type that returns client on auth failure

### Design: Fix async-imap Usage

#### 1. Update Email Channel Implementation

```rust
// src/channels/email.rs

/// Poll IMAP for new messages
async fn poll_imap(&self, bus: &MessageBus) -> Result<()> {
    use async_imap::Session;

    // Build TLS connector
    let tls = native_tls::TlsConnector::builder()
        .build()
        .map_err(|e| ChannelError::ConnectionFailed(format!("TLS setup failed: {}", e)))?;

    // Connect to IMAP server (new API - direct connection)
    let client = async_imap::connect(
        (self.config.imap_host.as_str(), self.config.imap_port),
        &self.config.imap_host,
        &tls,
    )
    .await
    .map_err(|e| ChannelError::ConnectionFailed(format!("IMAP connection failed: {}", e)))?;

    // Login (handle Result<Session, (Error, Client)>)
    let mut session = client
        .login(&self.config.imap_username, &self.config.imap_password)
        .await
        .map_err(|(e, _client)| {
            ChannelError::ConnectionFailed(format!("IMAP login failed: {}", e))
        })?;

    // Select INBOX
    session
        .select("INBOX")
        .await
        .map_err(|e| ChannelError::SendFailed(format!("Failed to select INBOX: {}", e)))?;

    // Search for unseen messages
    let unseen = session
        .search("UNSEEN")
        .await
        .map_err(|e| ChannelError::SendFailed(format!("Search failed: {}", e)))?;

    debug!("Found {} unseen email(s)", unseen.len());

    for seq_num in unseen.iter() {
        // Fetch message (API unchanged)
        let messages = session
            .fetch(format!("{}", seq_num), "(UID BODY.PEEK[])")
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Fetch failed: {}", e)))?;

        if let Some(fetch) = messages.iter().next() {
            let uid = fetch.uid.map(|u| u.to_string()).unwrap_or_default();

            // Check if already processed
            {
                let processed = self.processed_uids.read().await;
                if processed.contains(&uid) {
                    continue;
                }
            }

            // Parse message
            if let Some(body) = fetch.body() {
                if let Err(e) = self.process_email_body(body, &uid, bus).await {
                    error!("Failed to process email: {}", e);
                }

                // Mark as processed
                {
                    let mut processed = self.processed_uids.write().await;
                    processed.insert(uid.clone());
                    if processed.len() > 10000 {
                        processed.clear();
                    }
                }

                // Mark as seen (corrected API)
                let _ = session
                    .store(format!("{}", seq_num), "+FLAGS (\\Seen)")
                    .await;
            }
        }
    }

    // Logout
    let _ = session.logout().await;

    Ok(())
}
```

#### 2. Re-enable in Channel Manager

```rust
// src/channels/manager.rs

impl ChannelManager {
    pub async fn initialize_channels(&self) -> Result<()> {
        // ... other channels ...

        if self.config.channels.email.enabled {
            info!("Initializing Email channel");
            match EmailChannel::new(self.config.channels.email.clone()) {
                Ok(channel) => {
                    channels.insert("email".to_string(), Arc::new(channel));
                }
                Err(e) => error!("Failed to create Email channel: {}", e),
            }
        }

        // ... rest of channels ...
    }
}
```

#### 3. Verify async-imap Version

Ensure Cargo.toml has correct version and features:

```toml
async-imap = { version = "0.11.2", default-features = false, features = ["runtime-tokio"] }
```

---

## Feature 3: Built-in Skills Loading

### Current State Analysis
- `SkillManager::load()` only loads from workspace `skills/` directory
- TODO comment: "// TODO: Load built-in skills from bundled directory"
- Python nanobot has skills in `/nanobot/nanobot/skills/` with YAML frontmatter format
- Skills use `SKILL.md` files with metadata and content

### Design: Bundle and Load Built-in Skills

#### 1. Create Built-in Skills Directory

Create structure:
```
src/
  skills/              # Built-in skills (bundled at compile time)
    summarize/
      SKILL.md
    github/
      SKILL.md
    ...
```

Copy skills from Python project:
```bash
cp -r /path/to/nanobot/nanobot/skills/* src/skills/
```

#### 2. Use include_str!() Macro to Bundle Skills

```rust
// src/agent/skills.rs

/// Built-in skill definitions (bundled at compile time)
const BUILTIN_SKILLS: &[(&str, &str)] = &[
    ("summarize", include_str!("../skills/summarize/SKILL.md")),
    ("github", include_str!("../skills/github/SKILL.md")),
    ("weather", include_str!("../skills/weather/SKILL.md")),
    ("tmux", include_str!("../skills/tmux/SKILL.md")),
    ("cron", include_str!("../skills/cron/SKILL.md")),
    ("skill-creator", include_str!("../skills/skill-creator/SKILL.md")),
];

impl SkillManager {
    /// Load skills from workspace and built-in directories
    pub async fn load(&mut self, workspace_path: PathBuf) -> Result<()> {
        // Load built-in skills first
        debug!("Loading built-in skills");
        self.load_builtin_skills()?;

        // Load workspace skills (these override built-in skills)
        let workspace_skills_dir = workspace_path.join("skills");
        if workspace_skills_dir.exists() {
            debug!("Loading workspace skills from {:?}", workspace_skills_dir);
            self.load_from_directory(&workspace_skills_dir).await?;
        }

        debug!("Loaded {} skills total", self.skills.len());

        Ok(())
    }

    /// Load built-in skills from bundled content
    fn load_builtin_skills(&mut self) -> Result<()> {
        for (name, content) in BUILTIN_SKILLS {
            match self.parse_skill_content(name, content, PathBuf::from(format!("builtin::{}", name))) {
                Ok(skill) => {
                    debug!("Loaded built-in skill: {}", skill.name);
                    self.skills.insert(skill.name.clone(), skill);
                }
                Err(e) => {
                    warn!("Failed to load built-in skill '{}': {}", name, e);
                }
            }
        }
        Ok(())
    }

    /// Load a single skill file
    async fn load_skill(&self, path: &PathBuf) -> Result<Skill> {
        let content = fs::read_to_string(path).await?;
        let name = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        self.parse_skill_content(&name, &content, path.clone())
    }

    /// Parse skill content (shared by built-in and file-based loading)
    fn parse_skill_content(&self, name: &str, content: &str, path: PathBuf) -> Result<Skill> {
        // Parse frontmatter
        let (metadata, skill_content) = parse_frontmatter(content);

        let description = metadata
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let version = metadata
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0")
            .to_string();

        // Parse nanobot metadata if present
        let nanobot_meta = metadata
            .get("metadata")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|v| v.get("nanobot").cloned());

        let always = nanobot_meta
            .as_ref()
            .and_then(|m| m.get("always"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let triggers: Vec<String> = nanobot_meta
            .as_ref()
            .and_then(|m| m.get("triggers"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        let requires_bins: Vec<String> = nanobot_meta
            .as_ref()
            .and_then(|m| m.get("requires"))
            .and_then(|r| r.get("bins"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        let requires_env: Vec<String> = nanobot_meta
            .as_ref()
            .and_then(|m| m.get("requires"))
            .and_then(|r| r.get("env"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        // Check requirements
        let available = check_requirements(&requires_bins, &requires_env);

        Ok(Skill {
            name: name.to_string(),
            description,
            version,
            always,
            triggers,
            requires_bins,
            requires_env,
            path,
            content: Some(skill_content),
            available,
        })
    }
}
```

#### 3. Precedence: Workspace Skills Override Built-in Skills

The loading order ensures workspace skills override built-in skills with the same name:
1. Load built-in skills first → populate `skills` HashMap
2. Load workspace skills second → overwrite entries in `skills` HashMap if names match

#### 4. Directory Structure

**Option A: Store in `src/skills/`** (Recommended)
- Skills are bundled into the binary at compile time
- No external files needed at runtime
- Clean separation of built-in vs workspace skills

**Option B: Store in `skills/` at project root**
- Skills could be used as examples for users
- Easier to edit/test without recompiling
- Risk of confusion with workspace skills directory

**Recommendation: Use `src/skills/` for built-in skills**

---

## Implementation Order

1. **Email Channel Fix** (Easiest) - 1-2 hours
   - Update `email.rs` with correct async-imap API calls
   - Re-enable in `manager.rs`
   - Test with test email account

2. **Built-in Skills Loading** (Medium) - 2-3 hours
   - Create `src/skills/` directory
   - Copy skill files from Python project
   - Add `BUILTIN_SKILLS` const with `include_str!()` macros
   - Implement `load_builtin_skills()` and `parse_skill_content()`
   - Test loading and display

3. **Streaming LLM Responses** (Hardest) - 4-6 hours
   - Add streaming types to `types.rs`
   - Implement `chat_stream()` in `openai_compat.rs`
   - Update agent loop with streaming logic
   - Handle SSE parsing and tool call accumulation
   - Test with various LLM providers

## Testing Strategy

### Email Channel
```bash
# Manual test
klyntbot channels list
klyntbot channels test email test@example.com
```

### Built-in Skills
```bash
# Should show built-in skills even without workspace skills directory
klyntbot skills list
klyntbot skills info github
```

### Streaming
```bash
# CLI mode should show incremental output
klyntbot chat
> Write a long story about a robot
# Should see text appear incrementally, not all at once
```

---

## Success Criteria

### Email Channel Fix
- [ ] Email channel initializes without errors
- [ ] Can receive emails and publish to bus
- [ ] Can send email replies via SMTP
- [ ] No panics or crashes during IMAP polling

### Built-in Skills Loading
- [ ] All built-in skills load at startup
- [ ] Skills show in `klyntbot skills list`
- [ ] Workspace skills override built-in skills with same name
- [ ] Binary size increase is acceptable (<500KB for ~6 skills)

### Streaming LLM Responses
- [ ] Streaming works with OpenAI-compatible providers
- [ ] Content appears incrementally during generation
- [ ] Tool calls accumulate correctly across chunks
- [ ] Non-streaming providers fall back gracefully
- [ ] Agent loop completes successfully with both modes

---

## Risks and Mitigations

### Streaming
**Risk**: SSE parsing complexity and edge cases
**Mitigation**: Robust error handling, log malformed chunks, continue stream on parse errors

**Risk**: Tool call accumulation bugs
**Mitigation**: Thorough testing with multi-step tool use scenarios

### Email
**Risk**: async-imap API may have other incompatibilities
**Mitigation**: Check crate documentation, test thoroughly with real IMAP servers

### Built-in Skills
**Risk**: Binary size bloat
**Mitigation**: Keep skills concise, use compression if needed, measure binary size

**Risk**: Stale built-in skills
**Mitigation**: Document that workspace skills override built-in, provide update mechanism
