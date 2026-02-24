# Web Dashboard — Design

## Overview

A web-based dashboard for Klyntbot, served from the existing `klyntbot serve` command. New `dashboard` crate (Layer 4.5) provides an Axum HTTP server with REST API, WebSocket streaming, and an embedded React frontend. Developed as a localhost web app first; desktop (Tauri) conversion deferred to a future phase.

---

## Architecture

### New Crate: `dashboard` (Layer 4.5)

**Dependencies:** `agent`, `storage`, `config`, `common`, `tools-core`, `scheduling`

**External deps:** `axum`, `tower-http` (CORS, static serving, compression), `include_dir`, `serde_json`

```
crates/dashboard/
├── Cargo.toml
├── src/
│   ├── lib.rs          # DashboardServer: start()/stop()
│   ├── router.rs       # Axum Router with all routes
│   ├── state.rs        # AppState (Repos, AgentLoop handle, Config, broadcast channels)
│   ├── ws.rs           # WebSocket handler: AgentEvent → JSON frames
│   ├── api/
│   │   ├── mod.rs
│   │   ├── chat.rs     # POST /api/chat/send, GET /api/chat/sessions
│   │   ├── tasks.rs    # CRUD /api/tasks, GET /api/tasks/:id
│   │   ├── plans.rs    # CRUD /api/plans, GET /api/plans/:id
│   │   ├── calendar.rs # GET /api/calendar/events, POST /api/calendar/sync
│   │   ├── cron.rs     # CRUD /api/cron
│   │   ├── skills.rs   # GET /api/skills, PATCH /api/skills/:name
│   │   ├── finance.rs  # Full finance CRUD (accounts, transactions, budgets, investments, goals)
│   │   ├── settings.rs # GET/PATCH /api/settings/:section
│   │   └── status.rs   # GET /api/status (agent health, provider info, uptime)
│   └── embed.rs        # include_dir! static file serving (release builds)
└── frontend/           # React app (Vite project)
    ├── package.json
    ├── vite.config.ts
    ├── index.html
    └── src/
        ├── app/
        │   ├── App.tsx
        │   ├── routes.tsx
        │   ├── components/Layout.tsx
        │   └── pages/ (10 pages)
        ├── lib/
        │   ├── api.ts       # REST client (fetch wrapper)
        │   ├── ws.ts        # WebSocket client + reconnection
        │   ├── hooks/
        │   │   ├── useAgent.ts  # WebSocket hook for chat streaming
        │   │   └── useApi.ts    # REST data fetching hook
        │   └── types.ts     # TypeScript types matching Rust structs
        └── styles/
            └── theme.css    # Codex dark theme
```

---

## Integration with `klyntbot serve`

The `serve` command already creates all required subsystems (`Repos`, `AgentLoop`, `MessageBus`, `CronService`, `ChannelManager`). The dashboard slots in as one more service:

```rust
let dashboard = dashboard::DashboardServer::new(
    config.gateway.clone(),   // host + port (default 0.0.0.0:18790)
    repos.clone(),
    agent_loop.clone(),
    cron_service.clone(),
    config.clone(),
);
let dashboard_handle = tokio::spawn(async move { dashboard.start().await });
```

`GatewayConfig { host, port }` — the existing placeholder config — becomes the dashboard bind address. No new CLI commands. `klyntbot serve --port 18790` serves everything: web UI, REST API, WebSocket, channels, cron, heartbeat.

---

## WebSocket Protocol

Single endpoint: `GET /ws` (upgrade to WebSocket).

### Client → Server

```json
{ "type": "chat.send", "sessionKey": "optional-key", "message": "user text" }
{ "type": "interaction.respond", "requestId": "uuid", "response": { "Completed": [...] } }
{ "type": "chat.cancel" }
```

### Server → Client

All `AgentEvent` variants serialize as typed JSON frames:

```json
{ "type": "event.content_chunk", "data": "streamed text" }
{ "type": "event.tool_start", "name": "todo", "args": {...} }
{ "type": "event.tool_end", "name": "todo", "success": true, "durationMs": 42 }
{ "type": "event.iteration_start", "iteration": 1, "max": 5 }
{ "type": "event.classification", "strategy": "tool_assisted", "confidence": 0.92, "source": "classifier" }
{ "type": "event.context_assembled", "totalTokens": 2048, "budget": 4096 }
{ "type": "event.execution_started", "engine": "ToolAssisted", "maxIterations": 5 }
{ "type": "event.confidence", "score": 0.85, "action": "respond" }
{ "type": "event.plan_step", "planId": "uuid", "stepIndex": 2, "result": "..." }
{ "type": "event.plan_completed", "planId": "uuid", "summary": "..." }
{ "type": "event.done", "content": "final response" }
{ "type": "event.error", "message": "what went wrong" }
{ "type": "interaction.request", "requestId": "uuid", "title": "...", "questions": [...] }
```

### Internal flow

1. Client sends `chat.send` → handler calls `AgentLoop::process_direct_streaming()`
2. Returns `StreamingHandle { event_rx, interaction_rx, cancel_token, handle }`
3. Two forwarding tasks:
   - **Event forwarder:** reads `event_rx` → serialize → WebSocket send
   - **Interaction forwarder:** reads `interaction_rx` → sends `interaction.request` through WebSocket → holds `response_tx` in a `HashMap<Uuid, oneshot::Sender<FormResponse>>` → when client sends `interaction.respond`, pops sender and delivers
4. `chat.cancel` → triggers `cancel_token.cancel()`

---

## REST API

CRUD endpoints for entities that don't need real-time streaming. All return JSON with camelCase keys.

| Resource | Endpoints | Backend Source |
|----------|-----------|---------------|
| Tasks | `GET/POST/PATCH/DELETE /api/tasks`, `GET /api/tasks/:id` | `TodoRepo` |
| Plans | `GET/POST/PATCH /api/plans`, `GET /api/plans/:id` | `PlanRepo` |
| Calendar | `GET /api/calendar/events`, `POST /api/calendar/sync` | `CalendarRepo` + sync adapter |
| Cron | `GET/POST/PATCH/DELETE /api/cron` | `CronRepo` via `CronService` |
| Skills | `GET /api/skills`, `PATCH /api/skills/:name` | `SkillManager` |
| Finance | `GET/POST/PATCH/DELETE /api/finance/{accounts,transactions,budgets,investments,goals}` | Finance repos |
| Settings | `GET /api/settings`, `PATCH /api/settings/:section` | `config::load()` / `config::save()` |
| Sessions | `GET /api/sessions`, `GET /api/sessions/:key` | `SessionRepo` |
| Status | `GET /api/status` | Provider info, storage stats, uptime |

### Authentication

None. Localhost-only by default (`127.0.0.1`). Users who set `gateway.host` to `0.0.0.0` accept the risk.

---

## Frontend

### Stack (from Figma design)

- React 19 + React Router 7
- Tailwind CSS v4 (Codex dark theme: `#0d0d0d` bg, `#10a37f` accent)
- Radix UI primitives (dialog, dropdown, tooltip, popover, etc.)
- Motion (framer-motion) for animations
- Recharts for finance charts
- Lucide React for icons
- Inter + JetBrains Mono fonts

### Pages (10 routes)

| Route | Page | Data Source |
|-------|------|-------------|
| `/` | Chat | WebSocket (`useAgent` hook) |
| `/tasks` | Task List | REST (`GET /api/tasks`) |
| `/tasks/:id` | Task Detail | REST (`GET /api/tasks/:id`) |
| `/plans` | Plans | REST + WebSocket (plan progress events) |
| `/calendar` | Calendar | REST (`GET /api/calendar/events`) |
| `/cron` | Cron Jobs | REST (`GET /api/cron`) |
| `/skills` | Skills | REST (`GET /api/skills`) |
| `/finance` | Finance (6 tabs) | REST (finance endpoints) |
| `/settings` | Settings (14 sections) | REST (`GET/PATCH /api/settings`) |
| `/setup` | First-Run Wizard | REST (settings + status) |

### Figma → Codebase Adaptations

| Figma Component | Reality | Adaptation |
|----------------|---------|------------|
| Task `template` badge | Not in DB | Remove |
| Task `attachment` count | Not in DB | Remove |
| Task subtask progress | `parent_id` exists, no precomputed count | Compute client-side from list response |
| Task time tracking timer | `time_spent_minutes` in DB, no active timer | Client-side timer, PATCH on stop |
| Finance FIRE calculator | No backend calculation | Client-side only (reads from finance repos) |
| Calendar views (day/week/month) | Events from CalendarRepo | Lightweight calendar grid component |
| Chat thinking phases | Maps to AgentEvent sequence | Wire directly to WebSocket events |
| Chat strategy badges + confidence | `ClassificationComplete { strategy, confidence }` | Wire directly |
| macOS traffic lights in Layout | Web mode, not desktop | Replace with standard top bar |

### Layout shell

- 48px narrow left nav rail with 7 nav items + settings (from Figma)
- Bottom status bar showing: model name, session key, token cost
- No macOS traffic lights (web mode) — standard top bar instead
- Ultra-dark theme throughout (`--codex-bg: #0d0d0d`)

---

## Development Workflow

### Two-server mode (development)

```bash
# Terminal 1: Rust backend with hot reload
cargo watch -x 'run -- serve --port 18790'

# Terminal 2: Vite dev server with HMR
cd crates/dashboard/frontend && npm run dev
# → http://localhost:5173 (proxies /api and /ws to :18790)
```

### Vite proxy config

```typescript
export default defineConfig({
  server: {
    proxy: {
      '/api': 'http://localhost:18790',
      '/ws': { target: 'ws://localhost:18790', ws: true },
    },
  },
});
```

### Embedded mode (release)

```bash
cd crates/dashboard/frontend && npm run build
cargo build --release  # include_dir! embeds frontend/dist/
```

Release binary serves React app from memory at `/`, API at `/api/*`. Single binary.

---

## Desktop Transition Path (Future — Not In Scope)

Documented to ensure architecture doesn't block it:

1. Add Tauri v2 to workspace
2. Tauri `setup()` starts Axum on random localhost port
3. Webview loads `http://localhost:{port}`
4. System tray via `tauri::SystemTray` — macOS menu bar
5. WebSocket API stays identical

---

## Out of Scope (YAGNI)

- Authentication / login
- Multi-user support
- Task templates
- File attachments on tasks
- Real-time collaborative editing
- Browser push notifications
- Mobile responsiveness
- Tauri/desktop integration (Phase 2)
