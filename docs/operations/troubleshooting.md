# Troubleshooting Guide

## 1. Common Issues

### Build Failures

**Missing toolchain or tools:**

Klyntbot requires: `rustup` (stable toolchain), `cargo-nextest`, `bun`, and `cargo-tauri` (Tauri CLI v2). Verify all are installed:

```bash
rustc --version          # Rust stable
cargo nextest --version  # cargo-nextest
bun --version            # bun (never npm for desktop-ui)
cargo tauri --version    # Tauri CLI v2
```

**Clippy warnings fail CI:**

The project enforces a zero clippy warnings policy. Run clippy before committing:

```bash
cargo clippy --workspace --all-targets --all-features
```

The `desktop` crate has pre-existing exceptions, but all other crates must pass cleanly.

**Email feature gate:**

The `email` feature is on by default and gates IMAP/SMTP dependencies in the `channels` crate. If you see linker errors related to `native-tls` or IMAP libraries, ensure system TLS libraries are installed, or disable the feature:

```bash
cargo build --workspace --no-default-features
```

### Config Errors

**Empty or malformed config:**

The config file at `~/.klyntbot/config.json` uses camelCase field names (enforced by `#[serde(rename_all = "camelCase")]`). A missing config file is fine (all fields have defaults), but malformed JSON will prevent startup.

Validate your config:

```bash
python3 -c "import json; json.load(open('$HOME/.klyntbot/config.json'))"
```

**Provider auto-detection:**

If the agent reports "no provider configured," check that at least one provider has an API key set. The agent auto-detects by checking providers in order: anthropic, openai, openrouter, deepseek, gemini, groq, vllm, zhipu, dashscope, moonshot, minimax, aihubmix. The first with a non-empty `apiKey` wins. Override with `agents.defaults.provider`.

### Test Failures

**All tests use ephemeral SQLite.** Tests call `StoragePool::connect_in_memory()` and require no external database. If tests fail with storage errors, ensure `tempfile` can create temporary directories.

**Run tests with nextest:**

```bash
cargo nextest run --workspace                      # All tests
cargo nextest run -p agent                         # Single crate
cargo nextest run -E 'test(session_persistence)'   # Pattern match
cargo test --workspace --doc                       # Doctests (nextest unsupported)
```

## 2. Known Gotchas

### StoragePool::from_existing() Skips Migrations

`StoragePool::from_existing()` wraps an already-opened `SqlitePool` and **does not run migrations**. It is intended only for pools that have already been migrated. Tests must use `StoragePool::connect_in_memory()`, which runs migrations automatically.

**Symptom:** "no such table" errors at runtime.
**Fix:** Ensure you are using `connect_in_memory()` in tests and `connect()` (which runs migrations) in application code.

### tauri.conf.json Uses npm but Project Requires bun

The Tauri configuration at `crates/desktop/tauri.conf.json` specifies `npm` in `beforeDevCommand`, but the project requires `bun` for the desktop-ui. Running `cargo tauri dev` may fail with `ENOENT` if npm is not installed.

**Workarounds:**

1. Start Vite manually in one terminal: `cd desktop-ui && bun run dev`
2. Then run `cargo tauri dev` in another terminal.
3. Or use browser-only dev mode: run `cargo run -p dev-api` + `cd desktop-ui && bun run dev` in two terminals, then open `localhost:1420`.

### Config Changes Require Desktop App Restart

Configuration is loaded once at startup. Changes to `~/.klyntbot/config.json` are not picked up until the desktop app is restarted.

### Dependency Inversion for New Tools

New tools that need access to agent-level context (e.g., session state, other agents) must use dependency inversion via `Arc<dyn Trait>` to avoid circular crate dependencies. Handler traits like `SpawnHandler` and `CronHandler` are defined in lower-layer crates and implemented in the `agent` crate (Layer 5). Directly importing agent types from a tool crate will cause circular dependency compilation errors.

### Email Feature Gate

The `email` feature (on by default) gates IMAP/SMTP dependencies in the `channels` crate. If you are building without email support or on a platform where native-tls is problematic, disable it explicitly.

### Timestamps Are UTC

All Rust-emitted timestamps use `chrono::Utc::now().to_rfc3339()`. In the frontend:

- **Never** `.slice()` ISO strings for display.
- **Always** parse via `new Date(isoString)` and use `toLocaleTimeString()`.
- Use the shared helper `formatTime()` from `desktop-ui/src/lib/dates.ts`.

### Backdrop-Filter CSS Issues

Two CSS gotchas in the desktop-ui:

1. **Never write raw `backdrop-filter: blur() saturate()`** -- the CSS minifier breaks it. Use Tailwind's `@apply backdrop-blur-* backdrop-saturate-*` utilities instead. The `glass-panel` class demonstrates the correct pattern.
2. **Parent `backdrop-blur` blocks child `backdrop-filter`** -- this is a browser compositing limitation, not a bug.

### Overflow Clipping

Never use `overflow-x-auto` or `overflow: hidden` on containers that have absolute-positioned dropdown children. The overflow property clips absolutely-positioned elements. Use React portals instead to render dropdowns outside the clipping container.

## 3. Dev Server Issues

### SSE Buffering by Vite Proxy

When using the browser-only dev mode (Vite at `:1420` + dev API at `:3456`), Server-Sent Events (SSE) from the dev API may be buffered by Vite's proxy layer. This can cause chat responses to appear all at once instead of streaming.

**Workaround:** Access the dev API directly at `localhost:3456` for SSE endpoints, or use `cargo tauri dev` which does not proxy through Vite.

### Port Conflicts

- Vite dev server: port `1420`
- Dev HTTP server: port `3456`

If either port is in use, you will see bind errors on startup. Kill existing processes or change the port in the respective configuration.

## 4. Frontend Issues

### Tailwind v4 Migration

The desktop-ui uses Tailwind CSS v4 with the new CSS-based configuration system:

- **No `tailwind.config.js`.** All theming is in `desktop-ui/src/styles/theme.css` via CSS variables and `@theme inline`.
- **Never hardcode hex/rgba values.** Use token utilities: `bg-surface-base`, `text-muted`, `border-border`.
- **Adding new visual patterns:** Add a CSS variable to `:root` first, register it in `@theme inline`, then use via Tailwind utility classes.

### CSS Variable Naming

Follow the existing naming conventions in `theme.css`. Color tokens use the pattern `--color-surface-*`, `--color-text-*`, `--color-border-*`. Breaking from this convention will cause Tailwind to not generate the expected utility classes.

### Package Manager

Always use `bun` for the desktop-ui, never `npm` or `yarn`:

```bash
cd desktop-ui && bun install    # Install dependencies
cd desktop-ui && bun run dev    # Dev server
cd desktop-ui && bun run build  # Production build
cd desktop-ui && bun run lint:fix  # Biome 2.0 auto-fix
```

## 5. Debugging Tips

### Tracing Setup

Klyntbot uses the `tracing` crate for structured logging. Control log verbosity with the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run               # All crates at debug
RUST_LOG=agent=trace,tools=debug cargo run  # Per-crate levels
RUST_LOG=warn cargo run                # Only warnings and errors
```

For MCP stdio transport, tracing output is redirected to stderr to keep stdout clean for JSON-RPC communication.

### Log Levels

- `error` -- Unrecoverable failures, misconfigured providers.
- `warn` -- Tool execution failures, failed MCP connections, permission denials.
- `info` -- Agent startup, tool registration counts, index creation.
- `debug` -- Individual tool calls, file operations, cache hits/misses.
- `trace` -- Full LLM request/response bodies, message bus traffic.

### Inspecting SQLite DB

The SQLite database is at `~/.klyntbot/data.db`. Inspect it with any SQLite client:

```bash
sqlite3 ~/.klyntbot/data.db ".tables"           # List tables
sqlite3 ~/.klyntbot/data.db ".schema todos"     # Show table schema
sqlite3 ~/.klyntbot/data.db "SELECT count(*) FROM sessions;"
```

LanceDB vector data is stored separately in `~/.klyntbot/lance/`. It is not a SQLite database and requires the LanceDB tooling or the application itself to inspect.

### Inspecting Config

Print the resolved active provider:

```bash
cat ~/.klyntbot/config.json | python3 -c "import json,sys; c=json.load(sys.stdin); print('Provider:', c.get('agents',{}).get('defaults',{}).get('provider','auto-detect'))"
```

## 6. FAQ

**Q: How do I add a new LLM provider?**
A: Set the API key in config: `"providers": { "anthropic": { "apiKey": "sk-..." } }`. The provider is auto-detected from which keys are present. Override with `agents.defaults.provider`.

**Q: Why does my tool get "PermissionDenied"?**
A: Check `tools.permissions` in your config. The tool's required permission level may exceed what is granted to the channel. CLI typically needs `admin` level for full access.

**Q: How do I restrict the agent to only read files?**
A: Set `"tools": { "restrictToWorkspace": true }` and configure channel permissions to `readOnly`.

**Q: Why are my chat responses not streaming?**
A: If using browser-only dev mode, SSE may be buffered by Vite's proxy. Use `cargo tauri dev` or access the dev API directly.

**Q: How do I reset all data?**
A: Remove or rename the data directory: `mv ~/.klyntbot ~/.klyntbot.bak`. The application will recreate it on next startup with fresh databases.

**Q: Why does `cargo tauri dev` fail with ENOENT?**
A: The Tauri config references `npm` but the project uses `bun`. Start Vite manually with `cd desktop-ui && bun run dev` first, then run `cargo tauri dev`.

**Q: How do I install a WASM plugin?**
A: Place the plugin directory (containing `manifest.json` and `plugin.wasm`) in `~/.klyntbot/plugins/`. The plugin manager discovers and loads plugins at startup.

**Q: Config changes are not taking effect?**
A: The desktop app loads config once at startup. Restart the app after editing `~/.klyntbot/config.json`.
