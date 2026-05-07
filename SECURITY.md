# Security Policy

Klyntbot is a local-first personal cognitive agent OS. It runs on a single user's machine, stores credentials and personal data locally, and integrates with multiple external services. We take security seriously and appreciate responsible disclosure.

---

## Supported Versions

Klyntbot is pre-1.0. Only the **latest release** and the `main` branch receive security fixes. Older pre-release versions will not be patched — please update.

| Version | Supported |
|---------|-----------|
| `main` (unreleased) | Yes |
| Latest tagged release | Yes |
| Older tags / pre-releases | No |

---

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Please use one of the following private channels:

1. **GitHub Security Advisory** (preferred) — open a private advisory at
   <https://github.com/KlyntLabs/klyntbot/security/advisories/new>.
   This gives us a private fork to develop a fix and lets us coordinate disclosure with you.

2. **Email** — send a report to **jayden.dangvu@gmail.com** with the subject line
   `[klyntbot security] <short description>`. PGP is not currently offered; please avoid sending exploit details to other addresses.

Please include, where possible:

- A description of the vulnerability and its impact
- Steps to reproduce, or a proof-of-concept
- The affected version (commit SHA if building from source) and macOS version
- Any suggested mitigation
- Whether you intend to publicly disclose, and on what timeline

---

## Response Timeline

| Stage | Target |
|-------|--------|
| Acknowledgement of report | **Within 48 hours** |
| Initial severity assessment | **Within 7 days** |
| Fix released — Critical | **Within 14 days** |
| Fix released — High | **Within 30 days** |
| Fix released — Medium | Next minor release |
| Fix released — Low | Next major release or as bandwidth allows |

These are best-effort targets while Klyntbot is maintained by a single person. We will keep you updated on progress and let you know if a target slips.

### Severity guidelines

We use the following rough severity rubric. The maintainer makes the final call.

- **Critical** — Remote code execution; exfiltration of API keys or memory database without local user interaction; sandbox escape from WASM plugin; credential disclosure across user boundaries.
- **High** — Local privilege escalation beyond the user's existing privileges; bypass of risk-tier confirmation gates for destructive actions; unauthenticated read of arbitrary local files via a Klyntbot-exposed surface.
- **Medium** — Information disclosure of non-credential data; denial of service against the local daemon; insufficient input validation that requires unusual conditions to exploit.
- **Low** — Issues requiring an already-compromised local environment; theoretical weaknesses without a practical exploit.

---

## Disclosure Policy

We follow **coordinated disclosure**:

1. You report privately via one of the channels above.
2. We acknowledge, triage, and develop a fix in a private advisory or branch.
3. We agree on a disclosure date with you (default: 90 days from report, or earlier if a fix ships).
4. We release the fix, publish the advisory (with credit to you unless you prefer anonymity), and request a CVE if applicable.
5. We notify users via release notes and `CHANGELOG.md`.

If a vulnerability is being actively exploited, we may shorten the embargo to ship a fix faster.

---

## Scope

The following are **in scope** for security reports:

- The Klyntbot Rust binary, MCP server, and Tauri desktop app
- WASM plugin sandboxing
- The dev HTTP server (`crates/desktop/src/dev_server/`)
- Credential handling (API keys, OAuth tokens, secrets stored under `~/.klyntbot/`)
- IPC surfaces (`#[klynt_command]` / `#[klynt_raw_command]`)
- Channel adapters (Telegram, Discord, Slack, Email) when misconfiguration leads to credential leakage or impersonation
- Process hardening primitives in `klynt_process_hardening`

The following are **out of scope** or are expected behavior, not vulnerabilities:

- API keys stored in plaintext inside `~/.klyntbot/config.json` — this is a single-user local app and full-disk encryption is the user's responsibility.
- The local SQLite database (`data.db`) being readable by other processes running as the same user.
- Logs in `~/.klyntbot/` containing prompt content or tool inputs.
- The dev HTTP server (`localhost:3456`) accepting unauthenticated requests when the user explicitly runs `cargo tauri dev` — it is a development tool.
- Issues reachable only by an attacker who already has code execution on the user's machine, unless they bypass an explicit Klyntbot security boundary (e.g. WASM sandbox, risk-tier confirmation).
- Vulnerabilities in third-party LLM providers (Anthropic, OpenAI, etc.) — please report those to the provider.
- Issues in dependencies are in scope only if Klyntbot's usage makes them exploitable in a way the upstream usage does not.
- Social engineering, physical attacks, and attacks against the user's other software.

If you're unsure whether something is in scope, report it — we'd rather triage and decline than miss a real issue.

---

## Safe Harbor

We will not pursue legal action against, or otherwise penalize, security researchers who:

- Make a good-faith effort to follow this policy
- Report privately and give us reasonable time to respond before public disclosure
- Avoid privacy violations, destruction of data, and disruption of services beyond what is necessary to demonstrate the issue
- Do not exploit a vulnerability beyond proof-of-concept
- Do not access, modify, or retain data belonging to others

If in doubt, contact us before testing.

---

## Recognition

With your permission, we credit researchers in:

- The relevant `CHANGELOG.md` entry
- The published GitHub Security Advisory

If you prefer to remain anonymous, just say so in your report.

---

## Questions

For non-vulnerability security questions (architecture, hardening recommendations, dependency policy), please open a [Discussion](https://github.com/KlyntLabs/klyntbot/discussions) rather than an issue.
