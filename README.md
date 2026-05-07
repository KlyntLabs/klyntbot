<h1 align="center">Klyntbot</h1>

<p align="center">
  <strong>A local-first personal cognitive agent OS for macOS.</strong><br>
  Long-term memory · self-reflection · multi-channel access — all running on your machine.
</p>

<p align="center">
  <a href="./LICENSE"><img alt="License: AGPL-3.0" src="https://img.shields.io/badge/license-AGPL--3.0-blue"></a>
  <img alt="Platform: macOS" src="https://img.shields.io/badge/platform-macOS-lightgrey">
  <img alt="Status: pre-1.0" src="https://img.shields.io/badge/status-pre--1.0-orange">
  <img alt="Rust 1.93+" src="https://img.shields.io/badge/rust-1.93%2B-orange?logo=rust">
  <a href="https://github.com/KlyntLabs/klyntbot/discussions"><img alt="GitHub Discussions" src="https://img.shields.io/badge/discuss-GitHub-181717?logo=github"></a>
</p>

> **Pre-1.0 / unstable.** APIs, schemas, and config formats may change between releases without migrations. Don't rely on Klyntbot for anything you can't afford to rebuild.

---

## What is Klyntbot?

Klyntbot is a personal cognitive agent OS built in Rust. It runs entirely on your local machine and is designed to **think, remember, act, and improve over time**.

- 🧠 **Cognitive memory** — semantic + episodic memory, FSRS-style decay, knowledge graphs, procedural rules
- 💻 **Local-first** — Tauri 2 desktop app; all data lives in SQLite (WAL) and LanceDB under `~/.klyntbot/`
- 🤖 **Real agent runtime** — budget-aware execution, tool calls, mid-loop compression, fabrication detection
- 🌐 **Multi-channel** — Telegram, Discord, Slack, Email, and [MCP](https://modelcontextprotocol.io) — all sharing the same memory and persona
- 🧩 **Extensible** — feature crates, skills, WASM plugins, and an MCP server that exposes tools to other AI clients

It is not a chat wrapper. It is a 39-crate Rust workspace organized into 9 strict layers, with a dedicated agent runtime, cognitive layer, and skill router as first-class primitives.

---

## Quick start

### Requirements

- **macOS** (Apple Silicon or Intel) — the only supported platform
- Rust **stable** ≥ 1.93 (`rustup install stable`)
- [`bun`](https://bun.sh) (`curl -fsSL https://bun.sh/install | bash`)
- `cargo install cargo-nextest tauri-cli@^2`

### Build

```bash
git clone https://github.com/KlyntLabs/klyntbot.git
cd klyntbot
cargo build --workspace
cd desktop-ui && bun install && cd ..
```

### Configure

Create `~/.klyntbot/config.json` with at least one provider:

```json
{
  "providers": {
    "anthropic": { "apiKey": "sk-ant-..." }
  }
}
```

> ⚠️ API keys are stored in plaintext on disk. This is intentional for a single-user local app — see [SECURITY.md](./SECURITY.md) for the threat model.

### Run

```bash
cargo tauri dev   # Full desktop app
```

Or browser-only dev mode:

```bash
# Terminal 1 — backend + dev HTTP server (:3456)
cargo tauri dev
# Terminal 2 — frontend
cd desktop-ui && bun run dev
# Open http://localhost:1420
```

For an isolated dev instance that doesn't touch production data:

```bash
echo 'KLYNTBOT_HOME=~/.klyntbot-dev' > .env
```

---

## What it can do

| Capability | Notes |
|---|---|
| **Tasks & projects** | First-class entities with deadlines, recurrence, and cron + skill composition |
| **Notes & knowledge** | Backlinks, knowledge graphs, flashcards (FSRS5) |
| **Finance & FIRE** | Budgeting, transactions, portfolios, Monte Carlo planning |
| **Productivity & coaching** | Focus sessions, activity analytics, behavior interventions |
| **Multi-channel chat** | Telegram, Discord, Slack, Email, desktop, MCP |
| **Learning & language** | Flashcard generation, language practice, exam tracking |
| **MCP server** | Exposes Klyntbot tools to Claude Code, Cursor, and other MCP clients |

Each capability is a self-contained **feature package** with its own tools, migrations, config, and health checks.

---

## Why Klyntbot

- **Architected, not glued together** — strict 9-layer crate hierarchy with upward-only dependencies
- **Cognitive layer is first-class** — memory, reflection, and learning are core, not retrofits
- **Extensible like an OS** — skills, feature crates, WASM plugins, MCP client + server
- **Honest local-first** — no cloud sync, no telemetry, no account, no SaaS

If you want a SaaS chatbot, this isn't it. If you want a real agent OS you can read, fork, and extend, keep going.

---

## Documentation

| | |
|---|---|
| 📖 **Architecture deep dives** | [`docs/architecture/`](./docs/architecture/) |
| 🛠 **Contributing & dev workflow** | [`CONTRIBUTING.md`](./CONTRIBUTING.md) |
| 🔒 **Security policy** | [`SECURITY.md`](./SECURITY.md) |
| 📓 **Release history** | [`CHANGELOG.md`](./CHANGELOG.md) |
| 💬 **Questions & ideas** | [GitHub Discussions](https://github.com/KlyntLabs/klyntbot/discussions) |
| 🐛 **Report a bug** | [Open an issue](https://github.com/KlyntLabs/klyntbot/issues/new/choose) |

> User-facing installation, configuration, plugin, and skill authoring guides are planned. For now, the architecture docs and `CLAUDE.md` are the source of truth.

---

## License

Klyntbot is licensed under [**AGPL-3.0**](./LICENSE).

**Why AGPL?** Klyntbot is built to be a personal agent OS, not a SaaS substrate. AGPL ensures that anyone who runs a modified version as a network service must publish their changes — keeping improvements in the commons rather than locked behind a hosted product. If you're building a personal tool, a self-hosted deployment, or a contribution back upstream, AGPL is unobtrusive. If you want to wrap Klyntbot into a closed-source hosted service, AGPL will be in your way — that's by design.

**Running Klyntbot locally on your own machine — including the bundled MCP server consumed by your local AI clients — does not trigger AGPL's network-service clause.** The clause activates when you offer modified Klyntbot to *other users* over a network.

For commercial licensing inquiries, contact **jayden.dangvu@gmail.com**.

---

## Status & expectations

Klyntbot is pre-1.0, single-maintainer, and built primarily for the maintainer's own use. We welcome contributions, bug reports, and feedback — but please don't expect SLAs, polished UX in every corner, or stable APIs yet. See [CONTRIBUTING.md](./CONTRIBUTING.md) for the realistic response time and how to file useful reports.

---

<p align="center"><sub>Made with Rust, Tauri, and an embarrassing amount of self-talk.</sub></p>
