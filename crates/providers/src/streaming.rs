//! Shared SSE (Server-Sent Events) streaming infrastructure.
//!
//! Both the Anthropic native and OpenAI-compatible providers parse SSE streams
//! from HTTP responses. This module extracts the common line-buffering and
//! event-parsing pipeline so each provider only needs to supply its own
//! chunk-parsing function.

use futures_util::StreamExt;

use common::{KlyntbotError, ProviderError, Result};

use crate::types::LlmStreamChunk;

/// Build an `LlmStream` from a [`reqwest::Response`] and a provider-specific parser.
///
/// The `parser` receives `(event_type, data)` for each SSE event:
/// - `event_type`: the value from the `event: ...` line, if any (`None` for
///   OpenAI-style streams that only emit `data:` lines).
/// - `data`: the value from the `data: ...` line.
///
/// The parser should return:
/// - `Ok(Some(chunk))` — emit a chunk to the consumer.
/// - `Ok(None)` — skip this event (e.g., `[DONE]`, `ping`, `message_start`).
/// - `Err(e)` — propagate an error to the consumer.
pub fn sse_chunk_stream<F>(response: reqwest::Response, parser: F) -> crate::types::LlmStream
where
    F: Fn(Option<&str>, &str) -> Result<Option<LlmStreamChunk>> + Send + 'static,
{
    let byte_stream = response.bytes_stream();

    let chunk_stream = byte_stream.scan(
        (String::new(), String::new()), // (line_buffer, current_event_type)
        move |(line_buffer, current_event), result| {
            let chunks_result = match result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    line_buffer.push_str(&text);

                    let mut chunks = Vec::new();
                    while let Some(newline_pos) = line_buffer.find('\n') {
                        let line = line_buffer.drain(..=newline_pos).collect::<String>();
                        let line = line.trim();

                        if line.is_empty() {
                            continue;
                        }

                        if let Some(event_type) = line.strip_prefix("event: ") {
                            *current_event = event_type.to_string();
                        } else if let Some(data) = line.strip_prefix("data: ") {
                            let event_type = if current_event.is_empty() {
                                None
                            } else {
                                Some(current_event.as_str())
                            };

                            match parser(event_type, data) {
                                Ok(Some(chunk)) => chunks.push(Ok(chunk)),
                                Ok(None) => {}
                                Err(e) => {
                                    tracing::warn!("Failed to parse SSE chunk: {}", e);
                                    chunks.push(Err(e));
                                }
                            }
                        }
                    }
                    chunks
                }
                Err(e) => vec![Err(KlyntbotError::Provider(ProviderError::Http(
                    e.to_string(),
                )))],
            };

            async move { Some(futures_util::stream::iter(chunks_result)) }
        },
    );

    Box::pin(chunk_stream.flatten())
}
