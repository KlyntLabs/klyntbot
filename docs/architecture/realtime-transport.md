# Realtime transport — architectural decision

**Status:** Decided (Sprint A, 2026-05-04)
**Decision:** Stay with Tauri native events (production) + dev-server SSE (browser dev). Do NOT introduce WebSocket.

## Context

Repeated UX-perception question: "should we use WebSockets so chat feels more realtime?"

## Reality

- **Tauri native** (`app.emit` → `listen`): runs over OS-native IPC (Mach ports / named pipes / Unix sockets). Sub-millisecond p50.
- **Dev-server browser mode** (port 3456): uses Server-Sent Events via `axum::response::sse::Sse`. Single-direction streaming; user input goes via Tauri commands or HTTP POST.
- **Hypothetical WebSocket**: would need HTTP/1.1 → TCP → Upgrade → WS framing; same wire latency as SSE, additional complexity, no user-perceptible benefit.

## Measurements

See `crates/desktop/benches/event_transport_latency.rs`:

| Transport | p50 | p99 |
|---|---|---|
| Tokio broadcast (closest in-process surrogate for native IPC) | < 200 µs | < 1 ms |
| Dev-server SSE | < 2 ms | < 10 ms |
| Hypothetical WebSocket | identical to SSE for this workload | identical |

## When to revisit

1. Klyntbot ships a remote-server agent with browser clients.
2. A new feature requires bidirectional realtime (collaborative editing, live cursors).
3. User input frequency exceeds 10 Hz from the UI to backend.

None of these apply today.

## Consequence

- No WebSocket layer is added to either Tauri or dev-server paths.
- A small refinement to dev-server SSE: keep-alive interval changed from default to 15s.
- Benchmark above stays in tree as ongoing evidence.
