# Contributing to Klyntbot

Thanks for your interest in contributing! Klyntbot is a personal cognitive agent OS, and it gets better when more people use, break, and improve it.

This guide covers how to get set up, what we expect from contributions, and how to send your first patch.

---

## Conduct

Be respectful. Assume good faith. Disagree on technical merits, not on people. Report any unacceptable behavior to **jayden.dangvu@gmail.com**.

---

## Ways to Contribute

- **Report bugs** — open an issue using the bug template
- **Request features or discuss ideas** — start a [GitHub Discussion](https://github.com/KlyntLabs/klyntbot/discussions) before opening a feature issue
- **Improve documentation** — typo fixes through full architecture docs
- **Triage issues** — reproduce reports, add labels, link duplicates
- **Write code** — see [Development workflow](#development-workflow) below
- **Share skills and plugins** — once those authoring guides land, we'll link them here

Newcomers: look for issues labeled [`good first issue`](https://github.com/KlyntLabs/klyntbot/labels/good%20first%20issue) and [`help wanted`](https://github.com/KlyntLabs/klyntbot/labels/help%20wanted).

---

## Reporting Bugs

Use the bug report template. Include:

- macOS version and architecture (Apple Silicon / Intel)
- Klyntbot version (commit SHA if building from source)
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs from `~/.klyntbot/` (scrub secrets first)

**Do not file security vulnerabilities as public issues.** See [SECURITY.md](./SECURITY.md).

---

## Prerequisites

Klyntbot supports **macOS only** (Apple Silicon and Intel). Linux and Windows are not supported.

- `rustup` with the Rust **stable** toolchain (MSRV 1.93)
- `cargo-nextest` — `cargo install cargo-nextest`
- `bun` — `curl -fsSL https://bun.sh/install | bash`
- `cargo-tauri` v2 — `cargo install tauri-cli@^2`

---

## Local Setup

```bash
git clone https://github.com/KlyntLabs/klyntbot.git
cd klyntbot
cargo build --workspace
cd desktop-ui && bun install && cd ..
```

Run a dev instance with isolated config (does not touch your production `~/.klyntbot/`):

```bash
echo 'KLYNTBOT_HOME=~/.klyntbot-dev' > .env
cargo tauri dev                           # Backend + Tauri shell
cd desktop-ui && bun run dev              # Frontend (separate terminal)
```

For the broader architecture, read [`docs/architecture/`](./docs/architecture/).

---

## Development Workflow

### 1. Open an issue first (for non-trivial work)

Before sinking a weekend into a feature, please open an issue (or Discussion for open-ended ideas). Use your judgment — typo fixes, small bug fixes, and isolated docs improvements can skip the issue.

A "non-trivial" PR is roughly:

- New feature or new public API
- More than ~100 lines of changed Rust or TypeScript
- Schema, migration, or cross-crate refactor

### 2. Fork and branch

Branch names match commit prefixes:

```
feat/voice-input-streaming
fix/launcher-crash-on-empty-query
docs/contributing-guide
refactor/extract-task-handler
chore/bump-tokio
```

### 3. Make changes

- Match existing code style — don't reformat or "improve" adjacent code
- Add or update tests; tests are colocated as `#[cfg(test)] mod tests` inline, integration tests live in `tests/` via the facade crate
- Update documentation if you change behavior, configuration, or public APIs
- Keep PRs focused: one logical change per PR

### 4. Run all required checks locally

Every PR must keep these green:

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features   # zero warnings
cargo fmt --all --check
cd desktop-ui && bun run lint && bun run typecheck
```

If you touched the Tauri IPC surface, also run `cargo tauri dev` once so `desktop-ui/src/bindings.ts` regenerates.

### 5. Commit

Klyntbot uses **[Conventional Commits](https://www.conventionalcommits.org/)** — required, not optional. Format:

```
<type>(<scope>): <short description>
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`, `build`, `ci`.

Examples:

```
feat(tasks): add recurring task templates
fix(agent): prevent double-tool-call on cancellation
docs(readme): clarify macOS prerequisites
refactor(storage): extract MigrationRunner from StoragePool
```

### 6. Sign off (DCO)

Klyntbot uses the [Developer Certificate of Origin](https://developercertificate.org/) instead of a CLA. Every commit must be signed off:

```bash
git commit -s -m "feat(tasks): add recurring task templates"
```

This appends `Signed-off-by: Your Name <your@email>` to the commit message and certifies you wrote the code (or have the right to contribute it) under the project's license. Forgot to sign off? Amend:

```bash
git commit --amend -s --no-edit
```

You can configure git to auto-sign:

```bash
git config commit.gpgsign false  # if not using GPG
git config format.signoff true   # auto-add Signed-off-by
```

### 7. Open a pull request

- Use a Conventional-Commits-style title (it becomes the squash commit message)
- Link the related issue (`Closes #123`)
- Fill out the PR template
- For UI changes, attach before/after screenshots or a short screen recording
- Keep PRs under ~500 lines of diff where possible — split larger work into a stack

---

## Code Style

**Rust**

- Zero clippy warnings (the `desktop` crate has documented exceptions, don't add new ones)
- Use `common::Result<T>` (alias for `Result<T, KlyntbotError>`)
- Cross-crate imports use crate names directly: `use common::Result`, not `crate::`
- Public methods on `AppCore` handlers: `#[tracing::instrument(skip(self), err)]`
- Error handling: domain errors auto-convert via `From`; avoid one-off error types

**TypeScript / React**

- ESLint clean
- Use the project path aliases (`@/`, `@app/`, `@settings/`, `@threads/`, `@services/`, `@utils/`) — never `../../` relative imports
- Plain CSS — no Tailwind. Add new files via `@import` in `src/styles/index.css`
- Never hardcode `font-size: Npx` — use the `--fs-*` tokens in `ds-tokens.css`
- Tauri IPC: call `invoke()` directly from `@/api/client`; do not introduce `useQuery`/`useMutation` wrappers

**Tests**

- Use `StoragePool::connect_in_memory()` — no external DB needed
- For new Tauri commands, register via `#[klynt_command]` or `#[klynt_raw_command]` and add the path to `klynt_collect_commands![...]` in `specta_builder.rs`. Direct `#[tauri::command]` is rejected by a workspace test.

---

## Documentation

Documentation lives in:

- `README.md` — front door
- `CLAUDE.md` — guidance for AI pair-programming sessions
- `docs/architecture/` — system design and crate layout
- `docs/superpowers/` — design specs and plans
- `docs/coding-memory/` — coding-memory subsystem

Documentation PRs are welcome and do not require a prior issue.

---

## Discussions vs Issues

| Use Issues for | Use Discussions for |
|---|---|
| Reproducible bugs | "How do I…" questions |
| Concrete feature requests with a clear scope | Open-ended ideas |
| Tracked work with acceptance criteria | Show-and-tell (skills, plugins, configs) |
| Documentation gaps | Architecture debates |

If unsure, start in Discussions — we'll convert to an Issue when the scope is clear.

---

## Review Process

Klyntbot is currently maintained by a single person. We aim to acknowledge new issues and PRs **within one week**, though faster is the goal. A maintainer team is planned as the project grows.

What helps your PR land faster:

- Small, focused diffs
- Green CI (once CI lands; for now, green local checks)
- Linked issue with agreed-upon scope
- Tests for new behavior
- Self-review your own diff before requesting review

We may close PRs that:

- Sit without response from the author for >30 days
- Don't match agreed-upon scope from the linked issue
- Fail required local checks repeatedly without engagement

You're welcome to reopen and continue at any time.

---

## AI-Assisted Contributions

AI-assisted contributions are explicitly welcome — Klyntbot itself is built with heavy AI pair-programming. There is **no disclosure requirement**.

The bar is the same for AI-assisted and hand-written code:

- You understand every line you submit
- You can defend it in review
- You are responsible for its correctness, security, and license compatibility
- You sign it off under the DCO

Drive-by AI-generated PRs that the author cannot explain will be closed.

---

## License

Klyntbot is licensed under [AGPL-3.0](./LICENSE). By contributing, you agree that your contributions will be licensed under the same terms, and you certify the DCO on every commit.

---

Thank you for contributing. If anything in this guide is unclear, please open a Discussion — that's a contribution too.
