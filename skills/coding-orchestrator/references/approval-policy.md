# Approval Policy Reference

## Policy modes

| Mode | Behavior | When to use |
|------|----------|-------------|
| `askAlways` | Every bash command and file edit prompts for approval | Untrusted repos, learning |
| `askOnRisky` | 3-layer engine decides; only ambiguous cases ask | **Default** — most users |
| `askOnFailure` | Execute first, ask before retry on failure | Experienced users |
| `yoloMode` | Bypassed except privacy guard | CI/CD, automated workflows |

## 3-layer approval engine

When `askOnRisky` is active, each tool invocation passes through:

1. **Layer 1 — Declarative rules:** Per-tool allow/deny lists in config. Instant.
2. **Layer 2 — Starlark policies:** User-written `.star` rules in `.klyntbot/execpolicy/`. Flexible.
3. **Layer 3 — Mirror-learned:** Historically auto-approved patterns from past sessions. Adaptive.

If all layers defer, the request is escalated to the user (Ask).

## Approval decisions

When prompted, the user can:

| Decision | Effect |
|----------|--------|
| **Accept** | Execute this one time |
| **Decline** | Skip this action |
| **Accept for session** | Auto-approve similar requests for this thread |
| **Accept with execpolicy amendment** | Create a persistent Starlark rule |
| **Cancel** | Abort the entire turn |

## Privacy guard

Regardless of policy mode, paths matching `excludePaths` in config are **always denied**:
- `~/.ssh/`, `~/.gnupg/`, credential files
- `.env` files with secrets
- System directories outside workspace

The privacy guard cannot be bypassed by any approval decision.

## Layer decisions disclosure

When an approval card is shown, the UI displays the outcome of each layer:
- Privacy: passed/denied
- Layer 1: allowed/denied/deferred
- Layer 2: allowed/denied/deferred
- Layer 3: auto-allow/ask/deferred

This helps users understand why a request was escalated.

## Best practices

- Start with `askOnRisky` — it's the right balance of safety and speed
- Use `acceptWithExecpolicyAmendment` to build up rules over time
- Switch to `yoloMode` only for trusted, well-tested automation scripts
- Review `AGENTS.md` for workspace-specific approval conventions
