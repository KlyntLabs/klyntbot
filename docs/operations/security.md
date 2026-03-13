# Security Model

## 1. Overview

Klyntbot is a local-first AI agent that connects multiple chat platforms (Telegram, Discord, Slack, Email) to LLM providers with tool execution capabilities. Its security model is designed around the principle that the agent runs on a single user's machine, with all data stored locally. The primary threat model covers:

- Preventing LLM-driven tool calls from escaping intended boundaries (path traversal, command injection)
- Controlling which tools are available on which channels
- Protecting sensitive configuration values from accidental logging
- Sandboxing third-party WASM plugins
- Sanitizing inputs that flow into query predicates (LanceDB)

All network communication with LLM providers uses HTTPS. The agent does not expose any inbound network services in normal operation (MCP stdio transport redirects tracing to stderr).

## 2. Secret Management

### Secret\<T\> Wrapper

API keys and tokens are stored using the `Secret<T>` wrapper type defined in `crates/config/src/schema/core.rs`:

```rust
pub struct Secret<T>(T);
```

Key properties:

- **Debug and Display are redacted.** Both `fmt::Debug` and `fmt::Display` emit `[REDACTED]` instead of the inner value. This prevents accidental exposure in log output, error messages, and debug prints.
- **Explicit access required.** The inner value is only accessible via `.expose()` (borrow) or `.into_inner()` (consume). This makes secret access grep-able in code review.
- **Serde transparent.** Serialization/deserialization passes through to the inner type, so `Secret<String>` serializes as a plain string in JSON. This means the config file at `~/.klyntbot/config.json` contains API keys in plaintext.
- **Used throughout config.** All channel tokens (`TelegramConfig.token`, `DiscordConfig.token`, `SlackConfig.bot_token`, `SlackConfig.app_token`), provider API keys (`ProviderConfig.api_key`), and email passwords use `Secret<String>`.

### Plaintext at Rest

The configuration file (`~/.klyntbot/config.json`) stores secrets in plaintext. This is a deliberate simplicity tradeoff for a local-first single-user application (see ADR-009 in architecture decisions).

### Recommendations

- **File permissions.** Ensure `~/.klyntbot/config.json` is readable only by the owning user (`chmod 600`).
- **Keychain integration.** For production deployments, consider integrating with OS keychain (macOS Keychain, Linux Secret Service) to avoid plaintext secrets on disk.
- **Environment variables.** API keys can be set via environment variables using the override syntax: `KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=sk-...`. This avoids writing keys to the config file entirely.

## 3. Tool Permissions

### PermissionLevel Enum

Every tool declares a permission level via the `Tool::permission_level()` method. The four levels are defined in `crates/tools-core/src/permissions.rs`:

| Level | Value | Examples | Description |
|-------|-------|----------|-------------|
| `ReadOnly` | 0 | `read_file`, `list_dir`, `web_search`, `grep`, `glob` | Tools that only read data |
| `Standard` | 1 | `todo`, `project`, `memory`, `cron` | Tools that modify application state |
| `Elevated` | 2 | `write_file`, `edit_file`, WASM plugins with network/agent access | Tools that modify the filesystem or make external calls |
| `Admin` | 3 | `spawn` | Tools that create subagents or perform privileged operations |

Permission levels are ordered: a channel granted `Elevated` access can use `ReadOnly`, `Standard`, and `Elevated` tools, but not `Admin` tools.

### Per-Channel Permission Configuration

Permissions are configured in `config.json` under `tools.permissions`:

```json
{
  "tools": {
    "permissions": {
      "defaultLevel": "standard",
      "channels": {
        "cli": "admin",
        "telegram": "elevated",
        "discord": "standard",
        "slack": "readOnly"
      }
    }
  }
}
```

When `tools.permissions` is absent (the default), all tools are allowed on all channels for backward compatibility.

### Enforcement in ToolRegistry

Permission checks happen in `ToolRegistry::prepare()` (`crates/tools-core/src/registry.rs`), which is called before every tool execution:

1. Look up the tool by name (returns `ToolError::NotFound` if missing).
2. If permissions are configured, compare the tool's required `PermissionLevel` against the channel's granted level via `ToolPermissions::is_allowed()`. Returns `ToolError::PermissionDenied` if insufficient.
3. Validate the tool's parameters against its JSON Schema.
4. Return the tool for execution.

This separation of `prepare()` from `execute()` also prevents deadlocks when tools (like `delegate`) need to access the registry during execution.

## 4. Input Sanitization

### MCP Security

The MCP server module (`crates/mcp/src/server/security.rs`) provides two defenses for inputs received from external AI agents:

**Path traversal prevention** via `validate_path()`:
- Canonicalizes both the input path and the allowed base directory using `std::path::PathBuf::canonicalize()`.
- Verifies the canonical path starts with the canonical base. This resolves symlinks and `..` segments before comparison, preventing traversal attacks.
- Returns an error for paths outside the allowed base or paths that cannot be canonicalized (e.g., nonexistent).

**Input sanitization** via `sanitize_input()`:
- Strips all control characters except `\n` (newline) and `\t` (tab), which are legitimate in tool parameters.
- Truncates input to `MAX_INPUT_LENGTH` (50,000 characters) to prevent resource exhaustion.

### LanceDB Predicate Sanitization

LanceDB does not support parameterized queries, so string values interpolated into SQL filter predicates must be sanitized. The `sanitize_predicate_value()` function in `crates/storage/src/vector_store.rs`:

- **Rejects** values containing: semicolons (`;`), newlines (`\n`, `\r`), SQL comment markers (`--`, `/*`).
- **Escapes** single quotes by doubling them (`'` becomes `''`).
- Returns a `StorageError` for disallowed characters rather than silently modifying input.

This function is used in all `VectorStore` methods that build predicates: `upsert_embedding`, `delete`, `search_cognitive_facts`, and `dedup_table`.

## 5. Filesystem Tool Safety

### Workspace Restriction

The `restrict_to_workspace` flag in `ToolsConfig` (`crates/config/src/schema/tools.rs`) controls whether filesystem tools are confined to the configured workspace directory:

```json
{
  "tools": {
    "restrictToWorkspace": true
  }
}
```

When enabled, all filesystem tools (`read_file`, `write_file`, `edit_file`, `list_dir`, `grep`, `glob`) receive an `allowed_dir` parameter set to the workspace path. The `FsToolBase::resolve_path()` method in `crates/tools/src/filesystem.rs` enforces this by:

1. Expanding `~` via `shellexpand::tilde()`.
2. Canonicalizing the resolved path.
3. Checking that the canonical path starts with the canonical allowed directory.
4. Returning `ToolError::PermissionDenied` if the path falls outside.

### Browser Tool Write-Action Guards

The browser automation tool (`crates/tools/src/browser.rs`) implements a trust-level system to prevent unintended write actions:

- **Strict mode:** Guards every click, fill, type, and submit action with user confirmation.
- **Autonomous mode (default):** Guards only detected dangerous actions. Dangerous click targets include: submit, checkout, buy, purchase, confirm, place order, delete, remove, send, pay. Dangerous fill targets include payment fields: card number, CVV/CVC, expiry, billing.
- **Full mode:** No guards; all actions execute immediately.

### Permission-Gated Write Tools

Write-capable filesystem tools (`write_file`, `edit_file`) require `Elevated` permission. The `spawn` tool requires `Admin` permission. Read-only tools (`read_file`, `list_dir`) require only `ReadOnly` permission. Subagent profiles (`research`, `analyst`) register only read-only filesystem tools via `register_fs_read_tools()`.

## 6. Access Control

### Channel allow_from Lists

Every channel configuration includes an `allow_from` field that restricts which users can interact with the agent:

- **Telegram:** `channels.telegram.allowFrom` -- list of Telegram user IDs or usernames.
- **Discord:** `channels.discord.allowFrom` -- list of Discord user IDs.
- **Slack:** `channels.slack.allowFrom` -- list of Slack user IDs. Additionally, `channels.slack.dm.allowFrom` for DM-specific access, and `channels.slack.groupAllowFrom` for group channel access.
- **Email:** `channels.email.allowFrom` -- list of allowed sender email addresses.

When `allow_from` is empty, behavior depends on the channel implementation (typically allows all users).

### Per-Agent Tool and MCP Allowlists

Agent profiles (defined in `agents/` as Markdown with YAML frontmatter) specify which tools and MCP servers they can access:

- `mcp_tools` field controls MCP server access: `["*"]` grants access to all MCP servers, `[]` grants none, and specific server names can be listed.
- Tool availability is controlled by which tools are registered during agent initialization, based on the agent profile.

## 7. WASM Plugin Sandboxing

WASM plugins run inside the Extism runtime (`crates/plugin-runtime/`), which provides hardware-level sandboxing:

### Isolation Model

- **Memory isolation.** Each plugin runs in its own WASM linear memory. The `sandbox_memory_mb` manifest field controls the maximum memory allocation (converted to WASM pages of 64KB each).
- **No filesystem access.** Plugins cannot directly access the host filesystem. Storage operations go through host functions that check permissions.
- **Controlled host functions.** The plugin runtime exposes specific host functions with permission gates:
  - Storage functions check for `PluginPermission::Storage`.
  - Network functions check for `PluginPermission::Network`.
  - Agent functions (spawning subagents, etc.) check for `PluginPermission::Agent`.
  - Unauthorized calls return `"error: <type> permission denied"`.

### Permission Levels from Manifest

Plugin permission levels are automatically computed from the manifest (`crates/plugin-runtime/src/wasm_plugin.rs`):

- Plugins requesting `Network` or `Agent` permissions receive `PermissionLevel::Elevated`.
- All other plugins receive `PermissionLevel::Standard`.

This means channels with only `Standard` access cannot use plugins that declare network or agent permissions.

## 8. Recommendations

### Configuration Security

- Set file permissions on `~/.klyntbot/config.json` to `600` (owner read/write only).
- Use environment variable overrides for API keys in shared or automated environments.
- Restrict `allow_from` lists to known user IDs on all enabled channels.

### Operational Security

- Enable `restrictToWorkspace` to confine filesystem tools to the project workspace.
- Set per-channel permission levels appropriate to the trust level of each channel (e.g., `admin` for CLI, `standard` or `readOnly` for chat platforms).
- Use `Autonomous` or `Strict` trust level for the browser tool to prevent unintended write actions.
- Review WASM plugin manifests before installation, paying attention to declared permissions.

### Network Security

- All LLM provider communication uses HTTPS.
- MCP stdio transport does not expose network ports.
- Consider using a proxy (`channels.telegram.proxy`) for channels in restricted network environments.

### Future Improvements

- **Keychain integration.** Store API keys in OS keychain rather than plaintext config files.
- **Config file encryption.** Encrypt sensitive fields within `config.json` at rest.
- **Audit logging.** Log all tool executions with channel, user, and permission level for post-hoc review.
- **Rate limiting.** Per-channel rate limits to prevent abuse from compromised chat accounts.
