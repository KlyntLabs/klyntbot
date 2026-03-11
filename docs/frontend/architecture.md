# Frontend Architecture

## 1. Overview

The desktop UI is a React 19 single-page application rendered inside a Tauri 2 webview. It uses React Router 7 with hash-based routing, Tailwind CSS v4 for styling, and has no external state management library -- all state is local component state or lifted via props. The React Compiler (via `babel-plugin-react-compiler`) is enabled for automatic memoization.

The same codebase runs in two modes:

- **Tauri mode** -- embedded in the native desktop window, communicating with the Rust backend via Tauri's `invoke` IPC.
- **Browser dev mode** -- served by Vite on port 1420, communicating with a dev HTTP server on port 3456 via a Vite proxy.

## 2. Directory Structure

The project follows feature-based organization under `desktop-ui/src/`:

```
src/
  main.tsx                  # React entry point (StrictMode + createRoot)
  App.tsx                   # Root component (ThemeProvider + RouterProvider)
  app/
    router.tsx              # Hash router definition, lazy imports
    layouts/                # AppShell and other layout wrappers
    providers/              # ThemeProvider
  features/
    chat/                   # AI chat interface, streaming, message rendering
    dashboard/              # Day/week/month/year calendar views
    tasks/                  # Task & project management, OKR objectives
    notes/                  # Rich text notes (TipTap editor)
    finance/                # Accounts, transactions, budgets, investments, goals, liabilities
    settings/               # General, configuration, personalization, MCP, git, environments, integrations
    system/                 # Contexts, categories, inference, debug/events tabs
    setup/                  # First-run wizard (welcome, provider, channels, areas, etc.)
    tray/                   # System tray popover and launcher window
    distraction/            # Distraction detection overlay
    productivity/           # (Legacy, redirects to dashboard)
    debug/                  # (Legacy, integrated into system)
    work-contexts/          # Work context management
  shared/
    hooks/                  # useIpc, useQuery, useEvent, and other shared hooks
    lib/                    # utils.ts, dates.ts, errors.ts
    types/                  # Shared TypeScript type definitions
    styles/                 # Theme CSS files, glass utilities
  styles/
    index.css               # CSS entry point (imports fonts, tailwind, theme, prose, editor, themes, glass)
    theme.css               # Core design tokens and glass material definitions
    tailwind.css            # Tailwind v4 base import
    prose.css               # Markdown/prose typography
    editor.css              # TipTap editor styles
    fonts.css               # Font face declarations
```

## 3. Routing

Routing uses `createHashRouter` from React Router 7. All feature pages are lazy-loaded via `React.lazy()` with dynamic `import()`. The `AppShell` layout wraps all main routes and provides the sidebar navigation.

### Route Table

| Path | Component | Feature |
|---|---|---|
| `/` | `DashboardRedirect` | Redirects to `/day/{today}` |
| `/day/:date` | `DashboardLayout > DayCalendarView` | Daily timeline |
| `/week/:date` | `DashboardLayout > WeekCalendarView` | Weekly overview |
| `/month/:date` | `DashboardLayout > MonthCalendarView` | Monthly calendar |
| `/year/:year` | `DashboardLayout > YearHeatmapView` | Year heatmap |
| `/chat` | `ChatPage` | AI chat |
| `/tasks` | `TasksPage` | Task list |
| `/project/:id` | `ProjectDetailPage` | Project detail |
| `/task/:id` | `TaskDetailPage` | Task detail |
| `/objective/:id` | `ObjectiveDetailPage` | OKR objective detail |
| `/notes` | `NotesPage` | Notes editor |
| `/finance` | `FinanceOverviewPage` | Finance dashboard |
| `/finance/accounts` | `AccountsPage` | Bank accounts |
| `/finance/transactions` | `TransactionsPage` | Transaction list |
| `/finance/budgets` | `BudgetsPage` | Budget tracking |
| `/finance/investments` | `InvestmentsPage` | Investment portfolio |
| `/finance/goals` | `GoalsPage` | Financial goals |
| `/finance/liabilities` | `LiabilitiesPage` | Debts/liabilities |
| `/system` | `SystemPage` | System overview |
| `/system/:tab` | `SystemPage` | System tab (contexts, categories, events) |
| `/settings/general` | `SettingsLayout > GeneralSettings` | General settings |
| `/settings/configuration` | `SettingsLayout > ConfigurationSettings` | App configuration |
| `/settings/personalization` | `SettingsLayout > PersonalizationSettings` | Theme, appearance |
| `/settings/mcp` | `SettingsLayout > McpServersSettings` | MCP server management |
| `/settings/git` | `SettingsLayout > GitSettings` | Git integration |
| `/settings/environments` | `SettingsLayout > EnvironmentsSettings` | Environment variables |
| `/settings/integrations` | `SettingsLayout > IntegrationsSettings` | Third-party integrations |
| `/settings/archived` | `SettingsLayout > ArchivedSettings` | Archived items |
| `/setup/*` | `SetupLayout > *Step` | First-run wizard (welcome, provider, channels, areas, productivity, finance, mcp, complete) |
| `/launcher` | `LauncherPage` | Quick launcher popover |
| `/tray` | `SystemTrayPage` | System tray window |
| `/distraction-overlay` | `DistractionOverlay` | Fullscreen distraction alert |
| `*` | `SetupRedirect` | Checks `app_info` to route to setup or home |

### Setup Wizard Flow

The setup wizard at `/setup` is a standalone route tree outside the `AppShell`. It walks through: Welcome, Provider (LLM API keys), Channels, Areas, Productivity, Finance, MCP, and Complete.

## 4. Data Fetching

Data fetching is handled by a custom `useQuery` hook (`src/shared/hooks/useQuery.ts`) that provides SWR-style caching with request deduplication.

### Behavior

- **Stale-while-revalidate:** Cached data is served immediately. If the cache entry is older than `staleTime` (default: 30 seconds), a background refetch is triggered.
- **In-flight deduplication:** If a request for the same command + arguments is already in progress, the existing promise is reused instead of making a duplicate call.
- **Cache key:** Derived from the command name and JSON-serialized arguments. Pass `null` for `args` to skip fetching entirely.
- **Error handling:** Errors are parsed into structured `ApiError` objects (`{ code, message }`). Stale data is preserved on error.
- **Manual invalidation:** `invalidateQueries(cmdPrefix)` removes all cache entries whose key starts with the given prefix, forcing the next render to refetch.

### API

```typescript
function useQuery<T>(
  cmd: string,
  args?: Record<string, unknown> | null,
  fallback?: T,
  staleTime?: number,  // default 30_000ms
): { data: T; loading: boolean; error: ApiError | null; refetch: () => void }
```

## 5. IPC Abstraction

The `ipc()` function (`src/shared/hooks/useIpc.ts`) provides a single interface for calling backend commands that works transparently in both Tauri and browser environments.

### Detection

The `isTauri` constant (`src/shared/lib/utils.ts`) checks for the presence of `window.__TAURI_INTERNALS__`:

```typescript
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
```

### Dual-Mode Operation

- **Tauri mode:** Calls `invoke<T>(cmd, args)` from `@tauri-apps/api/core`.
- **Browser mode:** Sends `POST /api/{cmd}` with JSON body, proxied by Vite to `http://127.0.0.1:3456`.

The Vite config sets up the proxy:

```typescript
proxy: {
  "/api": { target: "http://127.0.0.1:3456", changeOrigin: true },
  "/attachments": { target: "http://127.0.0.1:3456", changeOrigin: true },
}
```

## 6. Event System

The `useEvent` hook (`src/shared/hooks/useEvent.ts`) subscribes to real-time events from the backend with automatic cleanup on unmount.

### Dual-Mode Operation

- **Tauri mode:** Uses `listen<T>(event, callback)` from `@tauri-apps/api/event`. Includes a `cancelled` guard to prevent duplicate event handling during React StrictMode's double-mount cycle.
- **Browser mode:** Listens for `CustomEvent` dispatched on `window`. SSE events from the dev server are bridged to `CustomEvent` by the streaming hook (see below).

### API

```typescript
function useEvent<T>(event: string, handler: (payload: T) => void): void
```

The handler is stored in a ref to avoid re-subscribing on every render.

## 7. Streaming

The `useAgentStream` hook (`src/features/chat/hooks/useAgentStream.ts`) manages real-time agent response streaming for the chat feature.

### Event Types

The hook listens to 20 SSE/Tauri events:

- `agent:content_chunk` -- incremental text from the LLM
- `agent:tool_start` / `agent:tool_end` -- tool execution boundaries
- `agent:done` / `agent:error` -- terminal events
- `agent:classification_complete` -- intent classification result
- `agent:execution_started` / `agent:iteration_start` -- execution engine metadata
- `agent:usage_report` -- token usage and cost
- `agent:memory_access` / `agent:skill_loaded` / `agent:learning_event` -- transparency data
- `agent:agent_selected` / `agent:subagent_spawned` -- agent routing
- `agent:delegation_started` / `agent:delegation_completed` -- inter-agent delegation
- `agent:plan_generated` / `agent:plan_step_completed` -- execution plan progress
- `agent:interaction_request` -- human-in-the-loop prompts
- `entity:updated` -- entity change notifications

### Performance: rAF Batching

Text chunks (`agent:content_chunk`) accumulate in a ref without triggering re-renders. The buffered text is flushed to React state at most once per `requestAnimationFrame` call. This prevents per-token re-renders during fast streaming.

### Tool Boundary Flushing

When a `tool_start` event arrives, any buffered text is immediately flushed and the text buffer is reset. This ensures text segments and tool segments appear in the correct order.

### Browser SSE Bridge

In browser dev mode, `startStreaming()` opens an `EventSource` connection to `http://127.0.0.1:3456/api/events/{sessionKey}` (bypassing the Vite proxy, which buffers SSE). Each SSE event is re-dispatched as a `CustomEvent` on `window`, where the `useEvent` listeners pick it up.

### Return Value

```typescript
interface AgentStream {
  segments: MessageSegment[];        // Accumulated text and tool segments
  isStreaming: boolean;
  activeTools: string[];             // Currently executing tools
  error: string | null;
  activeInteraction: ActiveInteraction | null;  // Human-in-the-loop request
  transparency: TransparencyData | null;        // Classification, usage, cost, tools, memory, etc.
  activeDelegateAgent: string | null;
  startStreaming: () => void;
  failStreaming: (message: string) => void;
  clearInteraction: () => void;
  clearSegments: () => void;
  clearTransparency: () => void;
}
```

## 8. Key Libraries

| Library | Version | Purpose |
|---|---|---|
| **React** | 19.x | UI framework |
| **React Router** | 7.x | Hash-based client-side routing |
| **TipTap** | 3.x | Rich text editor (notes feature). Extensions: code-block-lowlight, color, highlight, image, link, placeholder, subscript, superscript, table (cell/header/row), task-item, task-list, text-align, text-style, typography, underline |
| **Recharts** | 3.x | Chart components (finance, dashboard) |
| **D3-force** | 3.x | Force-directed graph layouts |
| **DnD Kit** | core 6.x, sortable 10.x | Drag-and-drop (task reordering, kanban) |
| **Radix UI** | checkbox, progress | Accessible primitives |
| **Lucide React** | 0.487.x | Icon library |
| **react-markdown** | 10.x | Markdown rendering with remark-gfm and rehype-highlight |
| **KaTeX** | 0.16.x | Math/LaTeX rendering |
| **clsx + tailwind-merge** | -- | Conditional class name composition via `cn()` utility |
| **lowlight** | 3.x | Syntax highlighting for code blocks (used with TipTap) |

## 9. Build Tooling

### Vite 6

- **Plugin: `@vitejs/plugin-react`** with React Compiler (`babel-plugin-react-compiler`).
- **Plugin: `@tailwindcss/vite`** for Tailwind CSS v4 integration.
- **Path aliases:** `@` = `src/`, `@shared` = `src/shared/`, `@features` = `src/features/`, `@app` = `src/app/`.
- **Build target:** `esnext`. Minification disabled in Tauri debug builds. Source maps enabled in debug.
- **Env prefix:** `VITE_` and `TAURI_` variables are exposed to client code.

### Biome 2.0

Biome handles linting, formatting, and import organization in a single tool. Configuration (`biome.json`):

- **Formatter:** 2-space indent, 100-character line width.
- **Linter:** Recommended rules enabled. Warnings for `noArrayIndexKey`, `noNonNullAssertion`, `noStaticElementInteractions`, `noImportantStyles`.
- **Import organization:** Automatic via `organizeImports: "on"`.
- **CSS:** Tailwind directive parsing enabled (`tailwindDirectives: true`).
- **Scope:** All files under `src/`.

### TypeScript 5.7

Strict mode. Type definitions for React 19, React DOM 19, D3-force, and KaTeX.

## 10. Development

### Setup

```bash
cd desktop-ui && bun install
```

Always use `bun`, never `npm` or `yarn`.

### Browser-Only Development

Run two terminals:

```bash
# Terminal 1: Rust dev server
cargo run -p dev-api

# Terminal 2: Vite dev server
cd desktop-ui && bun run dev
```

Open `http://localhost:1420` in a browser. The Vite proxy forwards `/api` and `/attachments` requests to the dev server on port 3456. SSE connections bypass the proxy and connect directly to `http://127.0.0.1:3456`.

### Full Desktop Development

```bash
cargo tauri dev
```

This starts both Vite and the Tauri app. Note: the `beforeDevCommand` in `tauri.conf.json` references `npm`, which may fail. Workaround: start Vite manually first (`cd desktop-ui && bun run dev`), then run `cargo tauri dev`.

### Linting and Formatting

```bash
cd desktop-ui && bun run lint:fix    # Biome: lint + format + import organization
cd desktop-ui && bun run lint        # Check only (no writes)
```

### Utility Functions

**`cn(...inputs)`** (`src/shared/lib/utils.ts`) -- Composes class names with `clsx` and deduplicates Tailwind classes with `tailwind-merge`.

**`formatTime(iso)`** (`src/shared/lib/dates.ts`) -- Formats an ISO timestamp to locale time (e.g., "14:30"). Always parse ISO strings via `new Date(iso)` and use `toLocaleTimeString()` -- never slice ISO strings for display.

**`todayISO()`** (`src/shared/lib/dates.ts`) -- Returns today's date as `YYYY-MM-DD` in local timezone.

**`parseApiError(e)`** (`src/shared/lib/utils.ts`) -- Narrows an unknown catch value to a structured `{ code, message }` object.

**`formatDuration(ms)`**, **`formatTokens(n)`**, **`formatCost(usd)`** (`src/shared/lib/utils.ts`) -- Human-readable formatting for durations, token counts, and USD costs displayed in the transparency panel.
