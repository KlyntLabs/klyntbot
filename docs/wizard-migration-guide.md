# Wizard Migration Guide

Guide for users upgrading to the enhanced klyntbot wizard.

## What Changed

### Original Wizard (v1)

The original wizard is a minimal 4-step setup:

1. **Provider selection** - Choose from 5 LLM providers
2. **API configuration** - Enter API key and model
3. **Channel prompt** - Yes/no question, with a hint to use `channels login` later
4. **Workspace setup** - Create directories and template files

**Limitations of v1**:
- No actual channel configuration (only shows a hint)
- No OAuth flows for Discord/Slack
- No daemon/service setup
- No tools configuration (Brave API key, exec permissions)
- No validation or connection testing
- No multi-provider setup in a single wizard run
- Basic text input only (no multi-select, no progress indicators)
- No checkpoint/resume (crash = restart from step 1)
- No back-navigation (can't go back to fix a mistake)

### Enhanced Wizard (v2)

The enhanced wizard is a comprehensive 7-phase guided setup:

| Phase | Name | What's New |
|-------|------|-----------|
| 1 | **Welcome & Overview** | Returning user detection, resume from checkpoint, configuration summary |
| 2 | **Provider Setup** | 12 providers (up from 5), API key format validation, connection testing, multi-provider support, custom endpoints |
| 3 | **Channel Configuration** | Full interactive setup for 8+ channels (Telegram, Discord, Slack, WhatsApp, Email, QQ, Feishu, DingTalk), OAuth browser flows, per-channel guided walkthrough, access control |
| 4 | **Tools & Permissions** | Brave Search API key, shell command timeout/allowlist, file access restrictions, workspace configuration |
| 5 | **Background Service** | Auto-detect macOS (launchd) or Linux (systemd), generate and install user-level service, gateway port configuration |
| 6 | **Validation & Testing** | Automated connection tests for all configured services, live test message, detailed error reporting with fix suggestions |
| 7 | **Summary & Next Steps** | Configuration recap, quick start commands, service control instructions |

**New features**:
- **Modular architecture** - Each phase is an independent `WizardModule` with `execute()`, `validate()`, and `rollback()` methods
- **Back-navigation** - Press `b` to go back to any previous step
- **Checkpoint/resume** - Progress saved to `~/.klyntbot/.wizard-state.json`; quit anytime and resume later
- **Progress bar** - Visual step indicator showing completed, current, and future phases
- **Interactive lists** - Arrow-key driven single-select and multi-select with fallback to numbered input
- **Secure input** - API keys masked with `●`, last 6 chars shown for verification
- **Inline validation** - Real-time format checking on API keys, tokens, URLs
- **Help tooltips** - Press `?` at any prompt for context-sensitive help
- **Confirmation cards** - Visual summary after each phase
- **NO_COLOR / accessibility** - Full support for colorless terminals, screen readers, narrow terminals

See the [Wizard Guide](wizard-guide.md) for the complete user walkthrough.

---

## Backward Compatibility

### Config File

The config file format (`~/.klyntbot/config.json`) is **fully backward compatible**:

- Same JSON schema with `camelCase` field names
- Same location (`~/.klyntbot/config.json`)
- Same minimal diff saving (only non-default values stored)
- New fields use `#[serde(default)]`, so old config files load without errors
- Existing API keys, channel tokens, and settings are preserved
- No migration script needed

### Workspace

Workspace files are **preserved** - the wizard's `create_template_file()` function only writes files that don't already exist:

```rust
fn create_template_file(path: &Path, content: &str) -> Result<()> {
    if !path.exists() {
        fs::write(path, content)?;
    }
    Ok(())
}
```

Your customized `AGENTS.md`, `SOUL.md`, `USER.md`, etc. are safe.

### CLI Commands

All existing CLI commands remain unchanged:

```bash
klyntbot init              # Still works, now runs enhanced wizard
klyntbot chat              # Unchanged
klyntbot serve             # Unchanged
klyntbot channels login    # Unchanged (wizard offers this inline too)
klyntbot config show       # Unchanged
klyntbot config validate   # Unchanged
klyntbot status            # Unchanged
```

### Environment Variables

All `KLYNTBOT_` environment variables continue to work identically.

### Model Names

The wizard now uses bare model names (e.g., `claude-sonnet-4-5`) without routing prefixes. If your existing config uses prefixed names like `anthropic/claude-sonnet-4-5`, they continue to work — the provider auto-detection matches model name keywords to route to the correct provider.

---

## How to Upgrade

### From an existing installation

Simply update the klyntbot binary. No migration steps needed:

```bash
# Your existing config is preserved
klyntbot status  # Verify your setup still works

# Optionally re-run the wizard for new features
klyntbot init
```

### Re-running the wizard

Running `klyntbot init` on an existing installation is safe:

- The wizard detects your existing config and shows a summary
- You can choose: **Reconfigure everything**, **Update specific settings**, or **Validate & test**
- Existing config values are loaded as defaults in input prompts
- Template files are not overwritten
- You can add new channels or providers without losing existing ones
- The wizard creates any missing directories
- The old config is backed up to `config.json.bak` before changes

### What "Update specific settings" does

This option lets you selectively re-run individual phases:

```
  What would you like to update?

    [ ] Provider configuration
    [✓] Channel configuration
    [ ] Tools & permissions
    [✓] Background service
    [ ] Run validation only

  ↑/↓ navigate  ·  Space toggle  ·  Enter confirm
```

Only the selected phases run. Unchecked phases are skipped entirely with existing config preserved.

---

## New Files

The enhanced wizard introduces these new files:

| File | Purpose | Persistence |
|------|---------|-------------|
| `~/.klyntbot/.wizard-state.json` | Checkpoint state for resume | Temporary (deleted on completion) |
| `~/.config/systemd/user/klyntbot.service` | Linux systemd unit file | Permanent (if daemon installed) |
| `~/Library/LaunchAgents/io.klyntbot.agent.plist` | macOS launchd plist | Permanent (if daemon installed) |

The wizard state checkpoint is only present while the wizard is in progress. It is automatically deleted when the wizard completes successfully.

---

## Configuration Examples

### Minimal config (CLI-only with Anthropic)

This is what both the v1 and v2 wizard produce for the simplest setup:

```json
{
  "agents": {
    "defaults": {
      "model": "claude-sonnet-4-5"
    }
  },
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-..."
    }
  }
}
```

### Multi-provider config

```json
{
  "agents": {
    "defaults": {
      "model": "claude-sonnet-4-5"
    }
  },
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-..."
    },
    "openai": {
      "apiKey": "sk-..."
    },
    "deepseek": {
      "apiKey": "..."
    }
  }
}
```

### Full multi-channel config with tools and daemon

This is the kind of config the v2 wizard produces for a power user:

```json
{
  "agents": {
    "defaults": {
      "model": "claude-sonnet-4-5",
      "maxTokens": 4096,
      "temperature": 0.5
    }
  },
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-..."
    }
  },
  "channels": {
    "telegram": {
      "enabled": true,
      "token": "123456:ABC...",
      "allowFrom": ["12345678"]
    },
    "discord": {
      "enabled": true,
      "token": "MTIz..."
    },
    "slack": {
      "enabled": true,
      "botToken": "xoxb-...",
      "appToken": "xapp-...",
      "mode": "socket"
    },
    "email": {
      "enabled": true,
      "imapHost": "imap.gmail.com",
      "imapPort": 993,
      "smtpHost": "smtp.gmail.com",
      "smtpPort": 587,
      "fromAddress": "user@gmail.com"
    }
  },
  "tools": {
    "web": {
      "braveApiKey": "BSA..."
    },
    "exec": {
      "timeout": 120,
      "allowedCommands": ["ls", "pwd", "cat", "git"]
    },
    "restrictToWorkspace": true
  },
  "gateway": {
    "host": "0.0.0.0",
    "port": 18790
  }
}
```

### Custom provider endpoint

```json
{
  "agents": {
    "defaults": {
      "model": "my-custom-model"
    }
  },
  "providers": {
    "openai": {
      "apiKey": "sk-...",
      "apiBase": "https://my-company-proxy.com/v1",
      "extraHeaders": {
        "X-Custom-Header": "value"
      }
    }
  }
}
```

---

## Feature Comparison

| Feature | v1 Wizard | v2 Wizard |
|---------|-----------|-----------|
| Providers | 5 (Anthropic, OpenAI, DeepSeek, Gemini, OpenRouter) | 12+ (adds Groq, vLLM, Zhipu, Dashscope, Moonshot, Minimax, AiHubMix, custom) |
| Channel setup | Hint only ("use `channels login`") | Full interactive setup for 8+ channels |
| OAuth flows | None | Embedded HTTP server for browser-based auth |
| Tools config | None | Brave API, shell timeout, command allowlist, workspace restriction |
| Daemon setup | None | systemd (Linux) and launchd (macOS) user-level services |
| Validation | None | API connection test, channel auth test, config validation |
| Live test | None | Optional test message through LLM and channels |
| Back-navigation | None | Press `b` to go back to any previous step |
| Checkpoint/resume | None | Auto-save progress, resume on next `klyntbot init` |
| Progress indicator | None | Visual progress bar with step labels |
| Input masking | None | API keys masked with `●`, last 6 chars shown |
| Format validation | None | Per-provider key prefix and length checks |
| Help system | None | Press `?` for context-sensitive help at any prompt |
| Multi-select | None | Arrow-key driven checkbox lists for channel selection |
| Returning user | None | Detects existing config, offers reconfigure/update/validate |
| Accessibility | Basic | NO_COLOR, non-TTY fallback, screen reader compatible, narrow terminal support |

---

## Getting Help

```bash
klyntbot --help           # General help
klyntbot init             # Re-run setup wizard
klyntbot status --verbose # Detailed system status
klyntbot config validate  # Check config file
klyntbot config show      # Show full effective config
klyntbot channels list    # See channel status
```

For detailed documentation:
- [Wizard Guide](wizard-guide.md) - Complete user walkthrough
- [Developer Guide](wizard-developer-guide.md) - Technical architecture and extension guide
- [Architecture Document](wizard-architecture.md) - Design decisions and module structure
- [UX Design Document](wizard-ux-design.md) - UI components and interaction specs
