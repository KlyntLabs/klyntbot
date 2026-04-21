# Seed: Deepening Cognitive / Feedback / Reforge Integration with Tasks

**Status:** Brainstorm seed — not a plan. Starting point for a future `/superpowers:brainstorming` session.

**Context:** After the removal refactor in `docs/superpowers/plans/2026-04-20-remove-builtin-ai-task-features.md` lands, the task system will be pure CRUD + scoring. Users compose AI behaviors themselves via cron + skills + the `agent` tool. The question this seed opens: **how should the remaining generic learning infrastructure (cognitive, feedback, reforge) plug into the slimmed-down task domain?**

---

## What is already wired (evidence from audit, 2026-04-20)

- `TaskCreated` published at `crates/feature-tasks/src/tool/actions/create.rs:182`.
- `TaskCompleted` published at `crates/feature-tasks/src/tool/actions/mutate.rs:281`.
- Cognitive background pipeline consumes both:
  - `crates/cognitive/src/services/background.rs:369` — `TaskCreated` upserts entity.
  - `crates/cognitive/src/services/background.rs:1013–1032` — `TaskCompleted` → episodic observation (plain text).
- Salience rules in `crates/cognitive/src/services/salience.rs:60–61`.
- Reforge reads two aggregate metrics from task data:
  - `crates/cognitive/src/services/reforge/collector.rs:98–107` — avg estimation deviation.
  - `crates/cognitive/src/services/reforge/collector.rs:137–148` — suggestion dismiss rate (will go stale after removal).

## What is missing

1. **No semantic-fact extraction from task completion.** Completing a task 3× over estimate should produce a fact like *"user underestimates `refactor` tasks by ~60%"* — today it's only a plain-text observation.
2. **No per-task episodic tagging.** Episodic entries carry the task ID nowhere structured; searching "what happened with task X" requires text matching.
3. **No mirror subscriber for task patterns.** Mirror watches routing/skills; tasks are invisible to it.
4. **No `tasks` rule domain in Reforge.** `RULE_DOMAINS` in `collector.rs` covers skill/routing/memory but not task handling, so Reforge can never generate procedural task-handling rules.
5. **No per-event feedback collector.** Skipped/rescheduled/late completions produce no structured feedback signal — only the estimation aggregate survives.
6. **Stale signal after removal.** `suggestion_dismiss_rate` reads from `task_suggestions`, which becomes inert once `suggest`/`apply_suggestion` are gone. Table can be dropped or repurposed.

## Open questions for the brainstorm

1. **What is the user-visible goal?** "The system learns your task habits" is vague. Concrete examples:
   - Surface an estimation-bias fact when the user plans their day via a user-authored automation.
   - Let Reforge suggest "you defer `admin` tasks 80% of the time; want to batch them?"
   - Feed episodic task history into the `agent` tool so user-authored "review my week" skills have structured recall.
   Pick 1–2 scenarios to drive the design.
2. **Where does feedback come from in the new model?** Users compose automations via cron + skills. Do those automations *explicitly* emit feedback events (user opts in), or do we infer from DB state (implicit)?
3. **Does Reforge gain a `tasks` rule domain, or does "task wisdom" live in the semantic-memory graph and get retrieved on demand?** Two very different architectures.
4. **What replaces `task_suggestions` as a feedback sink?** A generic `task_outcomes` table keyed by (task_id, event_type, timestamp) could feed both cognitive and reforge without the LLM-suggestion baggage.
5. **What's the minimum viable version?** E.g.: just emit `TaskDeferred` and `TaskReopened` events + add a `tasks` rule domain in Reforge. Everything else later.

## Design constraints (from CLAUDE.md and audit)

- Dependency inversion: any new handler trait lives in the consuming crate (`cognitive` or `autotuner`), not in `feature-tasks`. `feature-tasks` only *publishes* events.
- Pre-release — schema changes can be made directly (no migrations needed yet).
- Zero-clippy policy.
- Single-user local app: no observability infrastructure.

## Suggested brainstorm framing

Open the next session with: *"Given the task system is now pure CRUD + published events, what is the minimum set of structured signals, consumers, and stored facts that makes tasks a first-class citizen in the cognitive/feedback/reforge loop — and what single user-visible scenario would prove it works?"*

Reference this seed and the removal plan. Do not re-audit; the evidence above is current as of 2026-04-20.
