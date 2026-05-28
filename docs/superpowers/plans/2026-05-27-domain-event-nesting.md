# DomainEvent Per-Domain Nesting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure the flat 96-variant `bus::DomainEvent` into per-feature wrapper enums (`DomainEvent::Notification(NotificationEvent)`, `DomainEvent::Alarm(AlarmEvent)`, …), so a domain-focused subscriber matches one arm instead of N — while keeping every `variant_name()` / `domain()` / `KIND_*` output byte-identical.

**Architecture:** Behaviour-preserving structural nesting, migrated **one feature group per commit, compiler-guided**. Each group's variants move into a `#[serde(tag = "kind", rename_all = "snake_case")]` wrapper enum (mirroring the existing `TodoEvent` / `BashJobEvent`). `DomainEvent` gains a `Group(GroupEvent)` arm; `variant_name()` and `domain()` delegate into the wrapper, returning the **same** strings and `EventDomain` values as today. The 77 `KIND_*` constants stay on `DomainEvent` with unchanged values. A string-stability test (Task 0) locks all `KIND_*` values and rides green through every migration. Rust's exhaustiveness checking enumerates every publish/match site to fix.

**Tech Stack:** Rust, `serde`, `tokio::sync::broadcast`, `cargo-nextest`. No new dependencies (no macro/strum — `bus` has none and we are not deriving here).

**Source of truth:** `crates/bus/src/domain_events.rs` — the enum (`174–731`), `variant_name()` (`738–837`), `KIND_*` consts (`839–984`), `domain()` (`991–1096`), bus API (`1119–1150`), and the existing wrappers `TodoEvent`/`BashJobEvent` (`36–167`) as the pattern to copy.

---

## Design notes (decisions baked into this plan)

1. **Taxonomy = feature groups, not `EventDomain`.** Wrappers follow the `// -- Group --` comment structure (`Notification`, `Alarm`, `Task`, `Note`, …), like `TodoEvent`/`BashJob` — NOT the semantic `domain()` buckets (`Work`/`Energy`/`Learning`), which cut across features and even read a field (`UserStatedFact → Custom(domain)`).
2. **Only wrap groups with ≥2 variants.** Singletons (`ChatTurnCompleted`, `SkillRouted`, `LauncherItemExecuted`, `InterventionTriggered`, `CrossDomainDotReady`, `DataVersionBumped`, `PluginEvent`) and the existing escape variants (`Generic`, `Todo`, `BashJob`) **stay flat** — nesting one variant adds ceremony with no subscriber-side win (deletion test: a one-variant wrapper hides nothing).
3. **Strings and domains are frozen.** Each wrapper exposes its own `variant_name()` returning the exact prior string; `DomainEvent::variant_name()`/`domain()` delegate. `KIND_*` consts keep their current names AND values. Task 0 enforces this.
4. **Serde shape changes, but is not persisted.** `DomainEvent` is an in-process `broadcast` payload; no code serializes it to disk/wire (verified — zero `serde_json::*::<DomainEvent>` outside the defining file). The DB stores the `event_type` *string* (from `variant_name`/`KIND_`), which is frozen. So the wire-shape change is safe pre-1.0.
5. **One group per commit.** Each task compiles + passes the full suite on its own, so groups can be reviewed/merged independently.

### Wrapper inventory (the task list for the recipe)

| Wrapper enum | `DomainEvent` arm | Variants moved | Publish helper |
|---|---|---|---|
| `NotificationEvent` | `Notification` | HeldNotificationReleased, NotificationDeliveryFailed, TrayNotificationRequested | `publish_notification` |
| `AlarmEvent` | `Alarm` | AlarmFired, AlarmSnoozed, AlarmCancelled, MissedAlarms | `publish_alarm` |
| `TaskEvent` | `Task` | TaskCreated, TaskCompleted, TaskDeferred, TaskFocusChanged, TaskFocusExpired, EstimationRecorded | `publish_task` |
| `NoteEvent` | `Note` | NoteCreated, NoteUpdated, NoteContentChanged, NoteEditingFinished, NoteDeleted | `publish_note` |
| `ToolExecutionEvent` | `ToolExecution` | ToolCallExecuted, ApprovalRequested, ApprovalResolved | `publish_tool_execution` |
| `CoachingEvent` | `Coaching` | CoachingFeedback, CoachingStrategyApplied, CoachingPatternDetected, CoachingLearningDigest | `publish_coaching` |
| `CrossDomainEvent` | `CrossDomain` | UserStatedFact, UserCorrectedAI, AutotunerDecision | `publish_cross_domain` |
| `ProductivityEvent` | `Productivity` | ActivitySessionCompleted, FocusSessionStarted, FocusSessionEnded, ProductivitySessionEnded, DistractionDetected, ProductivityScoreComputed, SessionCreated, SessionEnded, QualityScored | `publish_productivity` |
| `LanguageLearningEvent` | `LanguageLearning` | PronunciationScored, ExamAttempted, PhoneticMasteryGained, LanguagePracticeSessionCompleted | `publish_language_learning` |
| `LifecycleEvent` | `Lifecycle` | SystemWillSleep, SystemDidWake, UserBecameIdle, UserReturned, FocusSessionSuspended, CronCatchUpReady, WakePanelReady | `publish_lifecycle` |
| `CommunityEvent` | `Community` | CommunityDiscovered, CommunityUpdated, CommunityWeakened, CoActivationStrengthened | `publish_community` |
| `CodingMemoryEvent` | `CodingMemory` | PatternApplied, PatternOutcome, FixAttemptFailed, MemoryRetrieved, AssistantMsgCompleted, RetrievalSkillApplied | `publish_coding_memory` |
| `LearningEvent` | `Learning` | BehavioralPatternDetected, ContradictionDetected, KnowledgeAtomCreated, KnowledgeAtomAccepted, KnowledgeAtomArchived, AtomFlashcardReviewed, AtomReinforced, KnowledgeAtomExtracted, FlashcardScheduled, AtomRetentionDecayed, AtomSemanticFactLinked, AtomInteracted, RetentionMilestoneReached, TranslationCompleted, NoteStudied, PracticeUnitCompleted, PracticeSessionCompleted, FlashcardSessionCompleted, KnowledgeTransferDetected | `publish_learning` |

> `LearningEvent` is the largest (19 variants) — migrate it **last**, after the recipe is proven on the smaller groups. Keep `KnowledgeAtomExtracted`'s mapping in mind: it appears in the cognitive ingest path.

---

## File Structure

- **Modify** `crates/bus/src/domain_events.rs` — add wrapper enums; replace flat variants with `Group(GroupEvent)` arms; update `variant_name()` + `domain()` to delegate; add `publish_<group>` helpers. Add `#[cfg(test)] mod string_stability;`.
- **Create** `crates/bus/src/string_stability_tests.rs` — the Task 0 anchor (or inline `#[cfg(test)] mod`).
- **Modify** (per group, compiler-flagged) the publisher + subscriber crates. Representative sites: `crates/scheduling/src/temporal/scheduler.rs` & `cron_executor.rs` (Alarm), `crates/notifications/src/dispatcher.rs` (Notification), `crates/feature-tasks/*` (Task), `crates/cognitive-memory/src/services/background.rs` (matches many), `crates/app-core/src/init/ai_pipeline.rs` (translator), `crates/app-core/src/wake_orchestrator.rs` (Lifecycle).

---

## Task 0: String-stability anchor (the TDD guard)

**Files:**
- Modify: `crates/bus/src/domain_events.rs` (add `#[cfg(test)] mod string_stability;` at the bottom)
- Create: `crates/bus/src/string_stability_tests.rs`

- [ ] **Step 1: Write the anchor test**

Create `crates/bus/src/string_stability_tests.rs`. Assert **every** `KIND_*` constant equals its current literal (copy all 77 from `domain_events.rs:839–984`), plus representative `variant_name()` / `domain()` round-trips. This test must stay green through every later task — it is how we prove the nesting never drifts a load-bearing string.

```rust
//! Frozen-string anchor for the DomainEvent nesting migration.
//! These strings feed DB `event_type` queries and MUST NOT change.
use super::*;

#[test]
fn kind_constants_are_frozen() {
    // Copy ALL 77 KIND_ constants from domain_events.rs. Sample shown;
    // include every one — the point is total coverage.
    assert_eq!(DomainEvent::KIND_TASK_CREATED, "TaskCreated");
    assert_eq!(DomainEvent::KIND_TASK_COMPLETED, "TaskCompleted");
    assert_eq!(DomainEvent::KIND_CHAT_TURN_COMPLETED, "ChatTurnCompleted");
    assert_eq!(DomainEvent::KIND_USER_CORRECTED_AI, "UserCorrectedAI");
    assert_eq!(DomainEvent::KIND_DISTRACTION_DETECTED, "DistractionDetected");
    assert_eq!(DomainEvent::KIND_NOTE_CREATED, "NoteCreated");
    assert_eq!(DomainEvent::KIND_ATOM_REINFORCED, "AtomReinforced");
    // … all remaining KIND_ constants …
}

#[test]
fn variant_name_and_domain_are_frozen_for_samples() {
    // One representative per group that will be wrapped.
    let n = DomainEvent::TrayNotificationRequested {
        title: "t".into(), body: "b".into(), alarm_id: None,
    };
    assert_eq!(n.variant_name(), "TrayNotificationRequested");
    assert_eq!(n.domain(), crate::EventDomain::Notifications);

    let a = DomainEvent::AlarmFired {
        fire_id: "f".into(), kind: "cron_job".into(), ref_id: None,
        payload_json: "{}".into(), fired_at_ms: 0,
    };
    assert_eq!(a.variant_name(), "AlarmFired");
    assert_eq!(a.domain(), crate::EventDomain::Scheduler);
}
```

Add at the bottom of `domain_events.rs`:

```rust
#[cfg(test)]
mod string_stability;
```

- [ ] **Step 2: Run it to verify it passes NOW (pre-migration baseline)**

Run: `cargo nextest run -p bus -E 'test(string_stability)'`
Expected: PASS (it documents the current strings before anything changes).

- [ ] **Step 3: Commit**

```bash
cargo fmt -p bus
git add crates/bus/src/domain_events.rs crates/bus/src/string_stability_tests.rs
git commit -m "test(bus): freeze DomainEvent event-type strings before nesting

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 1: Tracer — wrap `NotificationEvent` (3 variants)

The smallest clean group. Fully coded here; every later group repeats this exact shape.

**Files:**
- Modify: `crates/bus/src/domain_events.rs`
- Modify: compiler-flagged publishers/subscribers (expect `crates/notifications/*`, `crates/app-core/*`)

- [ ] **Step 1: Add the wrapper enum**

In `domain_events.rs`, near the other wrappers (after `TodoEvent`, ~line 167), add:

```rust
/// Notification-domain events. Carried by `DomainEvent::Notification`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NotificationEvent {
    HeldNotificationReleased { held_id: String, alarm_id: String, channels: Vec<String> },
    NotificationDeliveryFailed { alarm_id: String, channel: String, error: String, attempts: u32 },
    TrayNotificationRequested { title: String, body: String, alarm_id: Option<String> },
}

impl NotificationEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::HeldNotificationReleased { .. } => "HeldNotificationReleased",
            Self::NotificationDeliveryFailed { .. } => "NotificationDeliveryFailed",
            Self::TrayNotificationRequested { .. } => "TrayNotificationRequested",
        }
    }
}
```

- [ ] **Step 2: Replace the flat variants with the wrapper arm**

In the `DomainEvent` enum, DELETE the three `// -- Notifications --` variants (`domain_events.rs:606–627`) and add (near `Todo`/`BashJob`):

```rust
    Notification(NotificationEvent),
```

- [ ] **Step 3: Delegate `variant_name()` and `domain()`**

In `DomainEvent::variant_name()`, remove the three deleted arms and add:

```rust
            Self::Notification(e) => e.variant_name(),
```

In `DomainEvent::domain()`, replace the three `Self::HeldNotificationReleased { .. } | … => D::Notifications` arms with:

```rust
            Self::Notification(_) => D::Notifications,
```

- [ ] **Step 4: Add the publish helper (mirrors `publish_todo`)**

In `impl DomainEventBus` (after `publish_bash_job`, ~line 1147):

```rust
    pub fn publish_notification(&self, event: NotificationEvent) {
        self.publish(DomainEvent::Notification(event));
    }
```

- [ ] **Step 5: Compile `bus` and let errors enumerate the call sites**

Run: `cargo build -p bus`
Expected: PASS (the `bus` crate is now self-consistent).

Run: `cargo build --workspace 2>&1 | grep -A3 "error\[" | head -60`
Expected: compile errors at EVERY publisher/matcher of the three notification variants. Fix each:
- Construction `DomainEvent::TrayNotificationRequested { .. }` → `DomainEvent::Notification(NotificationEvent::TrayNotificationRequested { .. })` (or `bus.publish_notification(NotificationEvent::TrayNotificationRequested { .. })`).
- Match arms `DomainEvent::TrayNotificationRequested { .. } => …` → `DomainEvent::Notification(NotificationEvent::TrayNotificationRequested { .. }) => …`, or collapse domain-focused subscribers to `DomainEvent::Notification(e) => handle(e)`.

- [ ] **Step 6: Verify build, anchor, and suite**

```bash
cargo build --workspace            # 0 errors
cargo nextest run -p bus -E 'test(string_stability)'   # anchor still green
cargo nextest run -p bus -p notifications -p app-core  # touched crates
```
Expected: all PASS. If `string_stability` fails, a string drifted — fix the wrapper's `variant_name` to match the frozen value.

- [ ] **Step 7: Commit**

```bash
cargo fmt -p bus -p notifications -p app-core
git add -A
git commit -m "refactor(bus): nest notification events under DomainEvent::Notification

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2..13: Per-group migration (apply the recipe)

For EACH remaining row in the **Wrapper inventory** table, perform the recipe below as its own task + commit. The steps are identical to Task 1 — only the enum name, variant list, `DomainEvent` arm, `domain()` mapping, and publish-helper name change.

### The Migration Recipe (one group = one task = one commit)

1. **Add the wrapper enum** with `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)] #[serde(tag = "kind", rename_all = "snake_case")]` and a `variant_name(&self) -> &'static str` whose arms return the **exact current strings** (copy them from `DomainEvent::variant_name()`).
2. **Cut** the group's flat variants out of `DomainEvent` (verbatim — fields come with them) and **add** the `Group(GroupEvent)` arm near `Todo`/`BashJob`.
3. **Delegate** in `DomainEvent::variant_name()`: `Self::Group(e) => e.variant_name()`.
4. **Delegate** in `DomainEvent::domain()`: replace the group's arms with `Self::Group(_) => D::<SameDomainAsBefore>`. (Look up the prior mapping in `domain()` — do not change it. The one special case is `UserStatedFact` in `CrossDomainEvent`, whose `domain()` reads a field: keep `Self::CrossDomain(CrossDomainEvent::UserStatedFact { domain, .. }) => D::Custom(domain.clone())`, and map the other two CrossDomain variants as before.)
5. **Add** `publish_<group>(&self, event: GroupEvent)` to `impl DomainEventBus`.
6. **`cargo build --workspace`** and fix every compiler-flagged construction + match site (the compiler lists them all). Collapse domain-focused subscriber matches to a single `Self::Group(e) => …` arm where it reads cleanly.
7. **Verify**: `cargo build --workspace` (0 errors), `cargo nextest run -p bus -E 'test(string_stability)'` (green), `cargo nextest run` for the touched crates.
8. **Commit**: `refactor(bus): nest <group> events under DomainEvent::<Group>`.

- [ ] **Task 2:** `AlarmEvent` / `Alarm` — verify against `scheduling` (scheduler.rs, cron_executor.rs), `feature-alarms`, `feature-tasks`, `notifications`.
- [ ] **Task 3:** `TaskEvent` / `Task` — verify against `feature-tasks`, `cognitive-memory` (background.rs, event_log.rs uses `KIND_TASK_CREATED` — that's a const, unaffected).
- [ ] **Task 4:** `NoteEvent` / `Note` — verify against `feature-notes`, `cognitive-memory`.
- [ ] **Task 5:** `ToolExecutionEvent` / `ToolExecution` — verify against `approval`, `app-core`, `agent`.
- [ ] **Task 6:** `CoachingEvent` / `Coaching` — verify against `feature-coaching`, `cognitive-mirror`.
- [ ] **Task 7:** `CrossDomainEvent` / `CrossDomain` — **note the `UserStatedFact` field-read in `domain()`** (recipe step 4).
- [ ] **Task 8:** `ProductivityEvent` / `Productivity` — verify against `feature-productivity`, `app-core`.
- [ ] **Task 9:** `LanguageLearningEvent` / `LanguageLearning` — verify against `feature-language-learning`.
- [ ] **Task 10:** `LifecycleEvent` / `Lifecycle` — verify against `app-core/wake_orchestrator.rs`, `platform-*`.
- [ ] **Task 11:** `CommunityEvent` / `Community` — verify against `cognitive-graph`, `cognitive-memory`.
- [ ] **Task 12:** `CodingMemoryEvent` / `CodingMemory` — verify against `klynt-core`, `cognitive-memory`.
- [ ] **Task 13 (largest, do last):** `LearningEvent` / `Learning` (19 variants) — verify against `cognitive-memory`, `feature-learning`, `feature-notes`, `cognitive-graph`.

---

## Task 14: Final verification

- [ ] **Step 1: `DomainEvent` is now mostly delegation — sanity-read it.** `variant_name()` and `domain()` should be ~16 `Self::Group(e) => …` / `Self::Group(_) => …` arms plus the unwrapped singletons (`ChatTurnCompleted`, `SkillRouted`, `LauncherItemExecuted`, `InterventionTriggered`, `CrossDomainDotReady`, `DataVersionBumped`, `PluginEvent`, `Generic`, `Todo`, `BashJob`).

- [ ] **Step 2: Full workspace gates.**

```bash
cargo build --workspace
cargo nextest run --workspace          # incl. string_stability anchor
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```
Expected: build 0 errors; tests all pass; clippy 0 *new* warnings (pre-existing app-core debt aside); fmt clean.

- [ ] **Step 3: Final commit (fixups only, if any).**

```bash
git add -A
git commit -m "refactor(bus): finish DomainEvent per-domain nesting

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:** Every ≥2-variant feature group in the inventory has a task (Tasks 1–13); the string/`domain()` invariants are guarded by Task 0 and recipe steps 3–4; singletons are explicitly left flat (Design note 2); final gates in Task 14. ✓

**Placeholder scan:** Task 0's `kind_constants_are_frozen` says "include every one" — that is a *completeness instruction with the source cited* (all 77 are at `domain_events.rs:839–984`), not a vague TODO; the executor copies them. Tasks 2–13 use a single fully-specified recipe rather than re-printing identical steps 13× — acceptable for a mechanical, compiler-driven migration where the only per-task deltas are the (fully tabulated) enum name + variant list.

**Type consistency:** Wrapper enums use `variant_name(&self) -> &'static str` uniformly; `DomainEvent::variant_name()`/`domain()` delegate via `Self::Group(e) => e.variant_name()` / `Self::Group(_) => D::X`; bus helpers are `publish_<group>(GroupEvent)` matching the existing `publish_todo`/`publish_bash_job`. `#[serde(tag = "kind", rename_all = "snake_case")]` matches `TodoEvent`/`BashJobEvent`.

**Honest scope note:** This is a large mechanical migration (~24 crates, ~96 variants). Its safety rests on three things, in order: Rust exhaustiveness (finds every site), the Task-0 string anchor (catches the one thing the compiler can't — drifted event-type strings), and the existing test suite. The subscriber-side win is real only for domain-focused subscribers; cross-domain subscribers (e.g. `cognitive-memory`'s extract-everything consumer) keep wide matches — that is expected, not a defect.
