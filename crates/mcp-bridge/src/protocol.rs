use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// 1 MB cap, mirroring `coding-ingest::transport::MAX_PAYLOAD_BYTES`.
pub(crate) const MAX_FRAME_BYTES: u32 = 1 << 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeFrame {
    /// Tauri event name, e.g. "entity:updated", "provider:degraded".
    pub event: String,
    /// Arbitrary JSON payload — mirrors `AppEventEmitter::emit_event`'s second arg.
    pub payload: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame too large: {0} bytes (max {1})")]
    TooLarge(u32, u32),
    #[error("decode: {0}")]
    Decode(#[from] serde_json::Error),
}

/// Encode `frame` as 4-byte LE length + JSON body and write to `writer`.
/// Does not flush or shutdown.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    frame: &BridgeFrame,
) -> Result<(), FrameError> {
    let body = serde_json::to_vec(frame)?;
    let len_u32: u32 = u32::try_from(body.len())
        .map_err(|_| FrameError::TooLarge(u32::MAX, MAX_FRAME_BYTES))?;
    if len_u32 > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(len_u32, MAX_FRAME_BYTES));
    }
    writer.write_all(&len_u32.to_le_bytes()).await?;
    writer.write_all(&body).await?;
    Ok(())
}

/// Read one frame from `reader`. Returns:
/// - `Ok(Some(frame))` on success.
/// - `Ok(None)` on clean EOF *before* the length prefix.
/// - `Err(_)` for partial reads, oversize prefixes, or decode errors.
pub async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<Option<BridgeFrame>, FrameError> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(FrameError::Io(e)),
    }
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(len, MAX_FRAME_BYTES));
    }
    let mut body = vec![0u8; len as usize];
    reader.read_exact(&mut body).await?;
    let frame = serde_json::from_slice(&body)?;
    Ok(Some(frame))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrips_through_json() {
        let frame = BridgeFrame {
            event: "entity:updated".into(),
            payload: serde_json::json!({ "entityKind": "task", "id": "t1" }),
        };
        let bytes = serde_json::to_vec(&frame).unwrap();
        let back: BridgeFrame = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, frame);
    }

    #[test]
    fn frame_preserves_arbitrary_payload_shapes() {
        let frame = BridgeFrame {
            event: "provider:degraded".into(),
            payload: serde_json::json!({
                "provider": "anthropic",
                "reason": "rate_limit",
                "retryAfterSeconds": 30,
                "nested": { "a": [1, 2, 3] }
            }),
        };
        let s = serde_json::to_string(&frame).unwrap();
        let back: BridgeFrame = serde_json::from_str(&s).unwrap();
        assert_eq!(back, frame);
    }
}

#[cfg(test)]
mod framing_tests {
    use super::*;
    use tokio::io::{duplex, AsyncWriteExt};

    fn sample_frame() -> BridgeFrame {
        BridgeFrame {
            event: "entity:updated".into(),
            payload: serde_json::json!({ "entityKind": "note", "id": "n42" }),
        }
    }

    #[tokio::test]
    async fn write_then_read_roundtrips() {
        let (mut writer, mut reader) = duplex(4096);
        let frame = sample_frame();
        write_frame(&mut writer, &frame).await.unwrap();
        writer.shutdown().await.unwrap();
        drop(writer);

        let received = read_frame(&mut reader).await.unwrap();
        assert_eq!(received, Some(frame));

        // Next read on closed half returns Ok(None) — clean EOF.
        assert_eq!(read_frame(&mut reader).await.unwrap(), None);
    }

    #[tokio::test]
    async fn read_frame_returns_none_on_clean_eof_before_any_bytes() {
        let (writer, mut reader) = duplex(64);
        drop(writer);
        assert_eq!(read_frame(&mut reader).await.unwrap(), None);
    }

    #[tokio::test]
    async fn read_frame_errors_on_too_large_length_prefix() {
        let (mut writer, mut reader) = duplex(64);
        // Bogus length: 10 MB > 1 MB cap.
        writer.write_all(&(10_000_000u32).to_le_bytes()).await.unwrap();
        writer.shutdown().await.unwrap();
        drop(writer);

        let res = read_frame(&mut reader).await;
        assert!(matches!(res, Err(FrameError::TooLarge(_, _))), "got: {res:?}");
    }

    #[tokio::test]
    async fn write_frame_errors_on_oversize_payload() {
        let mut sink = Vec::<u8>::new();
        // 2 MB string > 1 MB cap.
        let huge = "x".repeat(2 * 1024 * 1024);
        let frame = BridgeFrame {
            event: "x".into(),
            payload: serde_json::Value::String(huge),
        };
        let res = write_frame(&mut sink, &frame).await;
        assert!(matches!(res, Err(FrameError::TooLarge(_, _))), "got: {res:?}");
    }

    #[tokio::test]
    async fn read_frame_errors_on_truncated_body() {
        let (mut writer, mut reader) = duplex(64);
        // Claim 100 bytes but only send 5.
        writer.write_all(&(100u32).to_le_bytes()).await.unwrap();
        writer.write_all(b"hello").await.unwrap();
        writer.shutdown().await.unwrap();
        drop(writer);

        let res = read_frame(&mut reader).await;
        assert!(matches!(res, Err(FrameError::Io(_))), "got: {res:?}");
    }
}
