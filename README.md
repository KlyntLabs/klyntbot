# Klyntbot — Personal Cognitive Agent OS

Klyntbot turns your machine into a **personal cognitive agent OS**:  
a local-first Rust agent with long-term memory, self-reflection, and multi-channel access to your digital life — without depending on the cloud.

- 🧠 **Cognitive memory** — Semantic and episodic memory, FSRS-style decay, knowledge graphs, and procedural rules.
- 💻 **Local-first desktop** — Built with Tauri 2 and React 19; all data lives locally in SQLite and LanceDB.
- 🤖 **Real agent runtime** — Budget-aware execution, tool calls, context assembly, and fabrication detection.
- 🧩 **Extensible by design** — Skills, feature packages, WASM plugins, and MCP client/server support.

---

## What is Klyntbot?

Klyntbot is not just an LLM wrapper.  
It is a **personal cognitive agent OS** built in Rust, designed to think, remember, act, and improve over time.

It includes:

- A dedicated agent runtime that understands **context**, **tools**, **budgets**, and **multi-step execution**.
- A cognitive layer with **semantic, episodic, and procedural memory**.
- A local-first desktop app and multi-platform chat integrations, all powered by the same core brain.

---

## Core Ideas

### Cognitive Operating System

- **Bi-temporal semantic facts** — Tracks both *when a fact was true* and *when the system learned it*.
- **FSRS-style memory decay** — Memories strengthen with use and fade naturally over time.
- **Self-reflection** — The agent observes its own behavior, learns patterns, and improves its strategies over time.

### Local-First by Default

- All data is stored locally in **SQLite (WAL)** and **LanceDB**.
- No cloud dependency is required for storage or core functionality.
- The desktop experience is powered by **Tauri 2** and **React 19**.

### A Real Agent Runtime

- **Budget-aware execution** — Normal / DeepThink / Ultra depth modes with explicit token and turn limits.
- **Context engine** — Dynamically assembles system prompt, history, memory, and tools based on strategy and budget.
- **Execute loop** — Handles LLM ↔ tool cycles, mid-loop compression, live context refresh, and streaming updates.

---

## What It Can Do

Klyntbot is designed to handle deep personal workflows, not just chat:

- ✅ **Tasks and projects** — Agentic execution, decomposition, forecasting, and planning.
- 📓 **Notes and knowledge** — Backlinks, knowledge graphs, flashcards, and spaced repetition.
- 💰 **Finance and FIRE** — Budgeting, transactions, portfolios, and Monte Carlo FIRE planning.
- 📈 **Productivity and coaching** — Focus sessions, activity analytics, behavior tracking, and interventions.
- 🌐 **Multi-channel chat** — Telegram, Discord, Slack, Email, and desktop, all sharing the same memory and persona.
- 🧠 **Learning and language** — Flashcard generation, learning insights, and language practice.

Each capability is packaged as a self-contained **feature package** with its own tools, migrations, config, and health checks.

---

## Why Klyntbot?

Klyntbot is built for power users, developers, and builders who want a real agent OS instead of another SaaS chatbot.

- **Architected, not glued together** — A 37-crate Rust workspace organized into 9 layers with strict upward-only dependencies.
- **Cognitive layer first** — Memory, reflection, and learning are core features, not add-ons.
- **Extensible like an OS** — Skills, feature crates, WASM plugins, and MCP integration.
- **Developer-friendly** — Clear architecture, testable layers, and a desktop + browser development workflow.

---

## Quick Start

### Prerequisites

```bash
rustup install stable
cargo install cargo-nextest tauri-cli@^2
curl -fsSL https://bun.sh/install | bash
```

### Build

```bash
git clone <repo-url> && cd klyntbot
cargo build --workspace
cd desktop-ui && bun install && cd ..
```

### Configure

Create `~/.klyntbot/config.json`:

```json
{
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-..."
    }
  }
}
```

### Run

**Browser dev mode:**

```bash
# Terminal 1 — backend + dev HTTP server
cargo tauri dev

# Terminal 2 — frontend
cd desktop-ui && bun run dev
# Open http://localhost:1420
```

**Full desktop app:**

```bash
cargo tauri dev
```

---

## Learn More

- **Architecture Overview** — system diagram, crate hierarchy, and message flow.
- **Agent Runtime** — execution loop, budgets, compression, and streaming.
- **Cognitive Memory** — FSRS decay, 12-factor relevance, mirror, and reforge.
- **Context Engine** — token budgets, memory retrieval, and history compression.
- **Desktop App** — AppCore, Tauri adapter, and React frontend.

See the `docs/architecture/` folder for the full deep dive.
