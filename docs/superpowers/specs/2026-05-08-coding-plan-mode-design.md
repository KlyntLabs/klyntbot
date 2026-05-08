# Coding Plan Mode — design

**Date:** 2026-05-08
**Status:** Spec (brainstorming complete; awaiting user review before plan)
**Phase:** 2.2 of the long-running-task roadmap
**Companion docs:**
- TodoWrite spec (foundation): [`2026-05-07-coding-todowrite-design.md`](2026-05-07-coding-todowrite-design.md) — §8 sketches the plan-mode integration this spec finalizes
- Comparative analysis: [`../notes/2026-05-07-long-running-task-comparative-analysis.md`](../notes/2026-05-07-long-running-task-comparative-analysis.md) — §7 Phase 2.2
- Permission gate: [`2026-05-05-unified-permission-gate-design.md`](2026-05-05-unified-permission-gate-design.md)
- TodoWrite implementation (shipped): commit `4de92d1ab feat: per-agent coding todo lists with concurrency validation, mirror integration, and frontend UI`

---

## 1. Motivation

The TodoWrite infrastructure that landed in commit `4de92d1ab` already includes plan-mode plumbing that is **inert by design**: `RoutingContext.plan_mode_active` and `plan_session_id` exist (always `false` / `None`); `validate_plan_mode_pending_only` is implemented and tested; `TodoEvent::PlanProposed` / `PlanRatified` are wired to the bus; `plan_mode.rs` provides event builders. The four ratify/edit/remove Tauri commands have shells but their app-core handlers are stubs. The frontend `PlanModeBanner.tsx` is a 15-line placeholder.

Plan mode is the missing keystone. Without it, the LLM cannot enter a "propose-then-ratify" workflow — every coding turn is execute-immediately. With it, the user gains a structured review point for multi-step work, and the cognitive layer gains a *plan vs. execution* signal pair that no comparator agent can mine because they lack the cognitive subsystem.

This spec finalizes the seven open questions from the TodoWrite spec §8 and turns them into a single shippable phase.

---

## 2. Goals & non-goals

### Goals

1. Provide a user-triggered plan mode (`/plan`) that flips the coding session's approval policy to a `PlanMode` variant.
2. Restrict `Edit` / `Write` / `MultiEdit` / `coding_shell` (write) to a single per-session plan file while plan mode is active; allow read-only tools normally.
3. Surface a review UI (`PlanModeBanner`) that lets the user inline-edit, remove, ratify, or cancel proposed todos.
4. Emit typed events (`TodoPlanRatified`, `TodoPlanCancelled`) so the cognitive layer can mine plan-vs-execution patterns.
5. Provide a `DynamicInjector` scaffold reusable for Phase 2.4 (hooks).
6. Propagate plan mode to subagents so the LLM cannot bypass restrictions by delegating.
7. Wire the four currently stubbed app-core handlers (`coding_todo_get`, `coding_plan_ratify`, `coding_plan_user_edit`, `coding_plan_user_remove`).

### Non-goals

- LLM-driven plan-mode entry (`EnterPlanMode` / `ExitPlanMode` tools). User is the source of truth; the LLM observes via `RoutingContext`.
- Plan files committed to the repo. Plan files live under `{KLYNTBOT_HOME}/plans/`.
- Cross-thread plan-mode sharing. Plan mode is per-session.
- Branchable plans / D-Mail revert. Phase 4 territory.
- A `/plan-cleanup` slash command for old plan files. Deferred.

---

## 3. Architecture overview

```
User types /plan in composer
    ↓
Composer detects leading "/plan" → invoke coding_plan_enter(thread_id)
    ↓
coding_plan_enter handler
    · derives slug from session auto-title
    · creates {KLYNTBOT_HOME}/plans/<date>-<slug>.md if absent
    · generates plan_session_id (uuid simple)
    · swaps Arc<RwLock<CodingApprovalPolicy>> → PlanMode { ... }
    · emits coding:plan_entered + injects one-shot system-reminder
    ↓
LLM next iteration sees:
    · RoutingContext.plan_mode_active = true (derived from policy)
    · DynamicInjector pushes the per-turn plan-mode reminder
    · attempts to Edit anywhere except plan_file_path → ClassifyHook rejects
    · attempts to call coding_todo with non-Pending status → validate_plan_mode_pending_only rejects
    · proposes pending items via coding_todo (existing path)
    ↓
PlanModeBanner renders proposed items
    User inline-edits (debounced) → coding_plan_user_edit
    User removes items → coding_plan_user_remove
    User clicks [Ratify & Execute] → coding_plan_ratify
        · clear proposed_in_plan_session tags
        · swap policy → Default
        · emit TodoPlanRatified + one-shot ratification reminder for next iteration
    User clicks [Cancel Plan] → coding_plan_cancel
        · soft-delete plan-tagged items
        · swap policy → Default
        · emit TodoPlanCancelled
```

### Reuses existing infrastructure

- `CodingApprovalPolicy` (refactored from struct → enum)
- `ApprovalGate::ClassifyHook` for the write restriction
- `RoutingContext.plan_mode_active` / `plan_session_id` (already on the struct)
- `DomainEventBus` for typed event publishing (already has `TodoEvent::PlanProposed` / `PlanRatified`)
- `LiveContextRefresher` + `ContextUpdateQueue` for per-turn injection
- `validate_plan_mode_pending_only` (already shipped, tested)
- `TodoRepo` (already shipped)
- `feature-coding-todo::plan_mode` (already shipped event builders)
- `klynt_command` Tauri-command macro
- Frontend `useThreadEvents` reducer + `todoStore`

### Net-new code

- `CodingApprovalPolicy` enum refactor + `PlanMode` variant `ClassifyHook` branch
- `DynamicInjector` trait + `InjectorRegistry` in `crates/bus/src/injection.rs`
- `PlanModeInjector` impl in `feature-coding-todo`
- `coding_plan_enter`, `coding_plan_cancel` Tauri commands + handlers
- Four app-core handler implementations (replacing existing stubs)
- `TodoEvent::PlanCancelled` variant
- `TodoRepo::clear_plan_session_tag`, `TodoRepo::soft_delete_plan_session`
- `SubagentBuilder::inherit_plan_mode`
- `PlanModeBanner.tsx` full buildout (replacing 15-line placeholder)
- `compute_ratify_counts` helper
- `kebab()` slug helper + untitled-rename background task

---

## 4. `CodingApprovalPolicy` enum refactor

### Today

`crates/approval/src/coding_policy.rs` defines `CodingApprovalPolicy` as a struct with `perms: Vec<Permission>` and a `ClassifyHook` impl. ~5 call sites in the codebase.

### Target

```rust
pub enum CodingApprovalPolicy {
    Default {
        perms: Vec<Permission>,
    },
    PlanMode {
        plan_session_id: String,        // 32-hex (uuid simple)
        plan_file_slug: String,         // e.g. "2026-05-08-add-grpc-transport"
        plan_file_path: PathBuf,        // {KLYNTBOT_HOME}/plans/<slug>.md
        perms: Vec<Permission>,         // unchanged baseline; carried over
    },
    YoloMode {
        until: jiff::Timestamp,
    },
}

impl CodingApprovalPolicy {
    pub fn default_with_perms(perms: Vec<Permission>) -> Self { Self::Default { perms } }
    pub fn is_plan_mode(&self) -> bool { matches!(self, Self::PlanMode { .. }) }
    pub fn plan_session_id(&self) -> Option<&str> { ... }
    pub fn plan_file_path(&self) -> Option<&Path> { ... }
}

impl ClassifyHook for CodingApprovalPolicy {
    fn classify(&self, req: &ApprovalReq) -> Classification {
        match self {
            Self::PlanMode { plan_file_path, .. } => classify_plan_mode(req, plan_file_path),
            Self::Default { perms } => classify_default(req, perms),
            Self::YoloMode { until } if jiff::Timestamp::now() < *until => Classification::Allow,
            Self::YoloMode { .. } => classify_default(req, &default_perms()),
        }
    }
}

fn classify_plan_mode(req: &ApprovalReq, plan_file_path: &Path) -> Classification {
    if is_write_tool(&req.tool_name) {
        match req.write_target_path() {
            Some(target) if target == plan_file_path => Classification::Allow,
            Some(_) | None => Classification::RejectWithSystemReminder(
                plan_mode_write_rejection_prose(plan_file_path),
            ),
        }
    } else if is_read_tool(&req.tool_name) {
        Classification::Allow
    } else {
        Classification::RejectWithSystemReminder(non_read_in_plan_mode_prose())
    }
}
```

**Write-tool whitelist** (`is_write_tool`):
- `Edit`, `Write`, `MultiEdit`, `NotebookEdit`
- `coding_shell` (always treated as a write tool — bash can mutate anything)
- MCP tools whose `Tool::approval_class()` is `Destructive`

**Read-tool whitelist** (`is_read_tool`):
- `Read`, `Grep`, `Glob`, `LSP::*`, `coding_todo`, `WebFetch`, `WebSearch`

`ApprovalReq::write_target_path()` is a new helper: returns the canonicalized absolute `PathBuf` for the file the call would write, or `None` for tools without a single target (e.g., `coding_shell`).

### Migration path

The struct → enum change is mechanical. Compiler walks every site:

1. `AppCore::new` constructs `Default { perms }` instead of `CodingApprovalPolicy { perms }`.
2. Each `policy.perms` access becomes a match.
3. `ClassifyHook` impl moves to the enum.
4. Tests in `coding_policy.rs` update to construct via `Default { ... }`.

Estimated: 5 call sites × ~10 minutes each = half a day, plus the new `PlanMode` branch.

### Storage

Policy lives as `Arc<RwLock<CodingApprovalPolicy>>` per coding session, keyed by `thread_id` in `AppCore::coding_policies: DashMap<String, Arc<RwLock<CodingApprovalPolicy>>>`. (This map exists today — only its values change shape.)

### `RoutingContext` derivation

`RoutingContext::for_thread(thread_id)` reads the policy and populates:
- `plan_mode_active = policy.is_plan_mode()`
- `plan_session_id = policy.plan_session_id().map(String::from)`

These are already fields on `RoutingContext`; today they are always `false` / `None`. No schema change.

---

## 5. Plan file lifecycle

### Path

```
{KLYNTBOT_HOME}/plans/<YYYY-MM-DD>-<kebab(auto_title)>.md
```

- `KLYNTBOT_HOME` resolves to `~/.klyntbot` in production, `~/.klyntbot-dev` in dev (existing convention).
- Date from `jiff::Timestamp::now().to_zoned(local).strftime("%Y-%m-%d")`.
- `kebab(s)` lowercases, replaces non-alphanumeric runs with `-`, trims leading/trailing `-`, caps length at 60 chars.

### Untitled fallback

If `title_service` has not yet finished when `/plan` fires:
- Slug = `<date>-untitled-<uuid8>` where `uuid8` is the first 8 hex chars of `plan_session_id`.
- A background task (spawned by `coding_plan_enter`) subscribes to `coding:thread_updated`. When the matching thread's title arrives, the task:
  1. Computes the new slug from the title.
  2. Renames the file: `tokio::fs::rename(old, new).await`.
  3. Updates the policy via `RwLock::write` to point at the new path.
  4. Emits `coding:plan_updated` so the UI re-renders the banner header.
  5. Self-terminates after one rename.

### Creation

`coding_plan_enter` ensures `{KLYNTBOT_HOME}/plans/` exists (`tokio::fs::create_dir_all`) and creates the file if absent with this stub:

```markdown
# Plan: <auto title>

**Created:** <YYYY-MM-DD HH:MM local>
**Plan session:** <plan_session_id>

## Goals



## Approach



## Tasks


```

If the file already exists (re-`/plan` in the same thread, or user pre-created it), it is not modified — `/plan` is idempotent.

### Cleanup

Plan files persist indefinitely. They are user-owned artifacts in `KLYNTBOT_HOME`. A `/plan-cleanup` slash command is deferred to a follow-up.

If the user deletes the plan file out of band while plan mode is active, the next `Edit` to `plan_file_path` recreates it (don't crash; `OpenOptions::create(true)` semantics).

---

## 6. `DynamicInjector` scaffold

### Trait

```rust
// crates/bus/src/injection.rs (new file)
pub trait DynamicInjector: Send + Sync {
    fn name(&self) -> &str;
    fn collect(&self, ctx: &RoutingContext) -> Vec<ContextUpdate>;
}

pub struct InjectorRegistry {
    injectors: Vec<Arc<dyn DynamicInjector>>,
}

impl InjectorRegistry {
    pub fn new() -> Self { Self { injectors: Vec::new() } }
    pub fn register(&mut self, injector: Arc<dyn DynamicInjector>) { self.injectors.push(injector); }
    pub fn collect_all(&self, ctx: &RoutingContext) -> Vec<ContextUpdate> {
        self.injectors.iter().flat_map(|i| i.collect(ctx)).collect()
    }
}
```

### Wire-up

`LiveContextRefresher::refresh` (in `crates/agent/src/execution/live_context_refresher.rs`) gains an `Arc<InjectorRegistry>` field. After draining `ContextUpdateQueue`, it calls `registry.collect_all(&ctx)` and appends results to the same priority lane (90% high-priority budget; existing extractive-summary fallback applies if over-budget).

### `PlanModeInjector` (first impl)

```rust
// crates/feature-coding-todo/src/injector.rs (new file)
pub struct PlanModeInjector {
    policies: Arc<DashMap<String, Arc<RwLock<CodingApprovalPolicy>>>>,
}

impl DynamicInjector for PlanModeInjector {
    fn name(&self) -> &str { "plan_mode" }

    fn collect(&self, ctx: &RoutingContext) -> Vec<ContextUpdate> {
        if !ctx.plan_mode_active { return vec![]; }
        let Some(policy_lock) = self.policies.get(&ctx.thread_id) else { return vec![]; };
        let policy = policy_lock.read();
        if let CodingApprovalPolicy::PlanMode { plan_file_slug, plan_file_path, .. } = &*policy {
            vec![ContextUpdate::SystemReminder(
                render::plan_mode_reminder(plan_file_slug, plan_file_path),
            )]
        } else { vec![] }
    }
}
```

`render::plan_mode_reminder` (already exists in `feature-coding-todo/src/render.rs` per the shipped TodoWrite work — extend if needed) produces:

```
<system-reminder>
Plan mode active. You may only Edit/Write to {plan_file_path}.
Other write tools are blocked. Use coding_todo to propose pending items
for the user to review. The user will ratify or cancel before execution.
</system-reminder>
```

### Phase 2.4 reuse

When hooks land:

```rust
pub struct HookInjector {
    pre_compact_scripts: Vec<PathBuf>,
    // ...
}

impl DynamicInjector for HookInjector {
    fn collect(&self, ctx: &RoutingContext) -> Vec<ContextUpdate> {
        // Spawn each script; pipe stdout into ContextUpdate::SystemReminder
    }
}
```

No refactor needed. `InjectorRegistry::register` adds the new injector at AppCore init.

### Registration

`InjectorRegistry` is constructed in `AppCore::new`, registered with `PlanModeInjector`, then passed by `Arc` to `LiveContextRefresher` at construction.

---

## 7. Subagent inheritance

`SubagentBuilder` exists at `crates/agent/src/subagent.rs`. Add:

```rust
impl SubagentBuilder {
    pub fn inherit_plan_mode(mut self, parent_policy: &CodingApprovalPolicy) -> Self {
        if let CodingApprovalPolicy::PlanMode {
            plan_session_id, plan_file_slug, plan_file_path, perms,
        } = parent_policy {
            self.policy = CodingApprovalPolicy::PlanMode {
                plan_session_id: plan_session_id.clone(),
                plan_file_slug: plan_file_slug.clone(),
                plan_file_path: plan_file_path.clone(),
                perms: perms.clone(),
            };
        }
        self
    }
}
```

Called from the spawn site (the place that constructs the subagent's `AppCore`-equivalent). The subagent's `RoutingContext.plan_mode_active` then reads `true` automatically.

A subagent's `coding_todo` writes still go to the subagent's own row (existing cross-agent mutation prevention is unchanged), but the row is tagged with the same `proposed_in_plan_session`. Ratification clears tags across the whole agent tree in one transaction (single SQL `UPDATE coding_todos SET proposed_in_plan_session = NULL WHERE thread_id = ? AND proposed_in_plan_session = ?`).

---

## 8. `/plan` slash command

### Frontend detection

`desktop-ui/src/features/coding/components/Composer.tsx` (or its existing slash-command handler) detects a leading `/plan` (with or without arguments — arguments are ignored for now). The composer:

1. Does not send the message to the LLM.
2. Invokes Tauri command `coding_plan_enter(thread_id)`.
3. Clears the composer.
4. Shows a transient toast: "Plan mode active."

Other slash commands to add at the same time:
- `/plan-exit` → `coding_plan_cancel`
- (Existing slash-command convention: a registry of `(name, handler)` pairs in the composer; add the two entries.)

### `coding_plan_enter` handler

```rust
async fn coding_plan_enter(thread_id: &str) -> Result<CodingTodoView> {
    // 1. Verify session is coding mode.
    let session = sessions_repo.get(thread_id).await?;
    if session.mode != SessionMode::Coding { return Err(NotCodingMode); }

    // 2. Resolve title and slug.
    let title = session.title.clone().unwrap_or_else(|| format!("untitled-{}", short_uuid()));
    let date = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::system()).strftime("%Y-%m-%d").to_string();
    let slug = format!("{date}-{}", kebab(&title));

    // 3. Build paths.
    let plans_dir = klyntbot_home().join("plans");
    tokio::fs::create_dir_all(&plans_dir).await?;
    let plan_file_path = plans_dir.join(format!("{slug}.md"));

    // 4. Create stub if absent.
    if !plan_file_path.exists() {
        tokio::fs::write(&plan_file_path, plan_stub(&title, &plan_session_id)).await?;
    }

    // 5. Generate session id, swap policy.
    let plan_session_id = uuid::Uuid::new_v4().as_simple().to_string();
    let new_policy = CodingApprovalPolicy::PlanMode {
        plan_session_id: plan_session_id.clone(),
        plan_file_slug: slug,
        plan_file_path: plan_file_path.clone(),
        perms: existing_perms(thread_id),
    };
    swap_policy(thread_id, new_policy).await;

    // 6. Spawn untitled-rename watcher if title was a fallback.
    if session.title.is_none() {
        spawn_rename_watcher(thread_id, plan_session_id.clone());
    }

    // 7. Emit events.
    bus.publish("coding:plan_entered", PlanEnteredEvent { thread_id, plan_session_id, plan_file_path });
    inject_one_shot_reminder(thread_id, "Plan mode active. Decompose the user's request into pending coding_todo items. Edits are restricted to the plan file.");

    coding_todo_get(thread_id).await
}
```

### Exit paths

| Trigger | Handler | Effect |
|---|---|---|
| `[Ratify & Execute]` button | `coding_plan_ratify` | Clear plan-session tags, swap policy → `Default`, emit `TodoPlanRatified`, inject ratification reminder |
| `[Cancel Plan]` button | `coding_plan_cancel` | Soft-delete plan-tagged items, swap policy → `Default`, emit `TodoPlanCancelled` |
| `/plan-exit` | `coding_plan_cancel` | Same as Cancel button |
| `/plan` while already in plan mode | `coding_plan_enter` | No-op; returns existing `CodingTodoView` |

---

## 9. App-core handlers

All four return `CodingTodoView`:

```rust
pub struct CodingTodoView {
    pub agents: HashMap<String, Vec<TodoItem>>, // keyed by agent_id
    pub plan_mode_state: Option<PlanModeView>,
}

pub struct PlanModeView {
    pub plan_session_id: String,
    pub plan_file_slug: String,
    pub plan_file_path: PathBuf,
    pub proposed_item_count: usize,
}
```

### `coding_todo_get`

```rust
pub async fn coding_todo_get(&self, thread_id: &str) -> Result<CodingTodoView> {
    let rows = self.todo_repo.list_for_thread(thread_id).await?;
    let plan_state = self.coding_policies.get(thread_id)
        .and_then(|lock| match &*lock.read() {
            CodingApprovalPolicy::PlanMode { plan_session_id, plan_file_slug, plan_file_path, .. } => {
                Some(PlanModeView {
                    plan_session_id: plan_session_id.clone(),
                    plan_file_slug: plan_file_slug.clone(),
                    plan_file_path: plan_file_path.clone(),
                    proposed_item_count: rows.iter()
                        .filter(|r| r.proposed_in_plan_session.as_deref() == Some(plan_session_id))
                        .map(|r| r.items.len()).sum(),
                })
            }
            _ => None,
        });
    Ok(CodingTodoView { agents: group_by_agent(rows), plan_mode_state: plan_state })
}
```

### `coding_plan_ratify`

```rust
pub async fn coding_plan_ratify(&self, thread_id: &str, plan_session_id: &str) -> Result<CodingTodoView> {
    // 1. Verify policy still in PlanMode with matching session id.
    let policy_lock = self.coding_policies.get(thread_id).ok_or(NoPolicy)?;
    {
        let policy = policy_lock.read();
        match &*policy {
            CodingApprovalPolicy::PlanMode { plan_session_id: p, .. } if p == plan_session_id => {}
            _ => return Err(PlanSessionMismatch),
        }
    }

    // 2. Read snapshot for ratify-counts diff (taken at coding_plan_enter time).
    let snapshot = self.plan_snapshots.remove(&plan_session_id.to_string());
    let final_rows = self.todo_repo.list_for_thread(thread_id).await?;
    let (ratified, edited, removed) = compute_ratify_counts(snapshot.as_ref(), &final_rows);

    // 3. Clear tags.
    self.todo_repo.clear_plan_session_tag(thread_id, plan_session_id).await?;

    // 4. Swap policy.
    let perms = existing_perms_for(thread_id);
    *policy_lock.write() = CodingApprovalPolicy::Default { perms };

    // 5. Emit events.
    self.bus.publish_todo(TodoEvent::PlanRatified {
        thread_id: thread_id.into(),
        plan_session_id: plan_session_id.into(),
        ratified_count: ratified,
        user_edited_count: edited,
        user_removed_count: removed,
        timestamp: jiff::Timestamp::now(),
    });
    self.bus.publish("coding:todos_updated", thread_id);
    self.inject_one_shot_reminder(thread_id, &format!(
        "Plan ratified by user. {ratified} items active. Begin execution."
    ));

    self.coding_todo_get(thread_id).await
}
```

### `coding_plan_user_edit`

```rust
pub async fn coding_plan_user_edit(
    &self, thread_id: &str, plan_session_id: &str, items_json: &str,
) -> Result<CodingTodoView> {
    // 1. Verify in plan mode with matching session id.
    self.assert_plan_mode(thread_id, plan_session_id)?;

    // 2. Parse items.
    let items: Vec<TodoItemInput> = serde_json::from_str(items_json)?;

    // 3. Validate (status must be pending, etc.).
    let validated = validate_write(/* ctx with plan_mode=true */, items)?;

    // 4. Compute diff against current row, emit StateChanged events.
    let prior = self.todo_repo.get(thread_id, "root").await?;
    let events = compute_diff(&prior, &validated);
    for e in events { self.bus.publish_todo(e); }

    // 5. Overwrite row preserving proposed_in_plan_session tag.
    self.todo_repo.upsert(thread_id, "root", &validated, Some(plan_session_id)).await?;

    // 6. Emit ui event, return view.
    self.bus.publish("coding:todos_updated", thread_id);
    self.coding_todo_get(thread_id).await
}
```

### `coding_plan_user_remove`

```rust
pub async fn coding_plan_user_remove(
    &self, thread_id: &str, plan_session_id: &str, item_ids: &[String],
) -> Result<CodingTodoView> {
    self.assert_plan_mode(thread_id, plan_session_id)?;
    let prior = self.todo_repo.get(thread_id, "root").await?;
    let remaining: Vec<TodoItem> = prior.items.into_iter()
        .filter(|i| !item_ids.contains(&i.id))
        .collect();
    let events = removed_items(&prior.items, &remaining); // emits TodoCancelled per dropped
    for e in events { self.bus.publish_todo(e); }
    self.todo_repo.upsert(thread_id, "root", &remaining, Some(plan_session_id)).await?;
    self.bus.publish("coding:todos_updated", thread_id);
    self.coding_todo_get(thread_id).await
}
```

### `coding_plan_cancel`

```rust
pub async fn coding_plan_cancel(&self, thread_id: &str) -> Result<CodingTodoView> {
    let policy_lock = self.coding_policies.get(thread_id).ok_or(NoPolicy)?;
    let plan_session_id = match &*policy_lock.read() {
        CodingApprovalPolicy::PlanMode { plan_session_id, .. } => plan_session_id.clone(),
        _ => return Err(NotInPlanMode),
    };

    self.todo_repo.soft_delete_plan_session(thread_id, &plan_session_id).await?;
    *policy_lock.write() = CodingApprovalPolicy::Default { perms: existing_perms_for(thread_id) };

    self.bus.publish_todo(TodoEvent::PlanCancelled {
        thread_id: thread_id.into(),
        plan_session_id,
        timestamp: jiff::Timestamp::now(),
    });
    self.bus.publish("coding:todos_updated", thread_id);
    self.coding_todo_get(thread_id).await
}
```

### `compute_ratify_counts`

Diffs original snapshot vs final list:

```rust
pub fn compute_ratify_counts(
    snapshot: Option<&Vec<TodoItem>>, final_items: &[TodoItem],
) -> (usize, usize, usize) {
    let snapshot = snapshot.cloned().unwrap_or_default();
    let snap_by_id: HashMap<_, _> = snapshot.iter().map(|i| (i.id.clone(), i)).collect();
    let final_by_id: HashMap<_, _> = final_items.iter().map(|i| (i.id.clone(), i)).collect();

    let mut ratified = 0;
    let mut edited = 0;
    let removed = snap_by_id.keys().filter(|id| !final_by_id.contains_key(*id)).count();

    for (id, final_item) in &final_by_id {
        match snap_by_id.get(id) {
            Some(orig) if orig.title == final_item.title
                       && orig.concurrency == final_item.concurrency
                       && orig.blocked_by == final_item.blocked_by => ratified += 1,
            Some(_) => edited += 1,
            None => edited += 1, // newly added by user counts as edited
        }
    }
    (ratified, edited, removed)
}
```

The snapshot is taken at `coding_plan_enter` time and stored in `AppCore::plan_snapshots: DashMap<String, Vec<TodoItem>>` keyed by `plan_session_id`. Cleared on ratify or cancel.

### New repo methods

```rust
impl TodoRepo {
    pub async fn clear_plan_session_tag(&self, thread_id: &str, plan_session_id: &str) -> Result<()>;
    pub async fn soft_delete_plan_session(&self, thread_id: &str, plan_session_id: &str) -> Result<()>;
}
```

`soft_delete_plan_session` deletes the rows entirely (the items only existed as a proposal; no execution history to preserve). If a row contains items not tagged with the cancelled session, those are preserved — but per the design, plan-mode rows only ever contain plan-session items, so this is defensive only.

### Tracing

Each handler annotated with `#[tracing::instrument(skip(self), err)]` per CLAUDE.md.

---

## 10. Tauri commands

Six total — two new, four wired (replacing existing stubs):

```rust
#[klynt_command]
pub async fn coding_plan_enter(thread_id: String) -> CodingTodoView { ... }

#[klynt_command]
pub async fn coding_plan_cancel(thread_id: String) -> CodingTodoView { ... }

// Wired (replace stubs)
#[klynt_command]
pub async fn coding_todo_get(thread_id: String) -> CodingTodoView { ... }

#[klynt_command]
pub async fn coding_plan_ratify(thread_id: String, plan_session_id: String) -> CodingTodoView { ... }

#[klynt_command]
pub async fn coding_plan_user_edit(thread_id: String, plan_session_id: String, items_json: String) -> CodingTodoView { ... }

#[klynt_command]
pub async fn coding_plan_user_remove(thread_id: String, plan_session_id: String, item_ids: Vec<String>) -> CodingTodoView { ... }

#[klynt_command]
pub async fn coding_plan_open_file(path: String) -> () { /* opens via `open` crate */ }
```

Add the three new command paths (`coding_plan_enter`, `coding_plan_cancel`, `coding_plan_open_file`) to `desktop_macros::klynt_collect_commands![...]` in `specta_builder.rs`. Run `cargo tauri dev` to regenerate `desktop-ui/src/bindings.ts`.

---

## 11. `PlanModeBanner.tsx` UI

Rendered at the top of `MessagePane` whenever `plan_mode_state` is `Some`. Subscribes to `todoStore` for live updates.

### Layout

```
┌─ Plan mode · 2026-05-08-add-grpc-transport.md ─────────────[X]─┐
│  Reviewing 5 proposed items                                     │
│                                                                 │
│  [≡] task_1  Set up tonic dependency           Sequential  [×] │
│  [≡] task_2  Define proto schema               Sequential  [×] │
│  [≡] task_3  Generate stubs (depends: task_2)  Sequential  [×] │
│  [≡] task_4  Wire server handlers              Sequential  [×] │
│  [≡] task_5  Write integration test            Sequential  [×] │
│                                                                 │
│  [+ Add item]                                                   │
│                                                                 │
│         [Ratify & Execute]    [Cancel Plan]                     │
└─────────────────────────────────────────────────────────────────┘
```

### Behaviour

- **Title row**: shows `plan_file_slug.md`. Clicking opens the file in the OS default editor via the `open` crate (Tauri command `coding_plan_open_file(path)`).
- **Each item row**:
  - `[≡]` drag handle for reorder (uses `react-dnd` or HTML5 drag-and-drop; existing `desktop-ui` patterns show no library — go HTML5 native).
  - Title field: click to edit inline (`contentEditable` or controlled `<input>`); on blur, fire debounced `coding_plan_user_edit` (500ms).
  - Concurrency pill: dropdown with `Safe / Sequential / Exclusive`; on change, fire `coding_plan_user_edit`.
  - `[×]`: removes the item; calls `coding_plan_user_remove(plan_session_id, [item.id])`.
- **Status pill omitted**: locked to Pending in plan mode by `validate_plan_mode_pending_only`.
- **`[+ Add item]`**: appends a new pending item with placeholder title; user types; on blur fires `coding_plan_user_edit` with the full new list.
- **`[Ratify & Execute]`**: shows lightweight inline confirmation ("Ratify N items?") with `[Confirm] [Cancel]`. On confirm, calls `coding_plan_ratify`. Banner disappears on next render (because `plan_mode_state` becomes `None`).
- **`[Cancel Plan]`**: same inline confirmation pattern; calls `coding_plan_cancel`.
- **`[X]` close button**: top-right; equivalent to Cancel Plan (with confirmation).

### Sticky positioning

Banner is `position: sticky; top: 0; z-index: var(--z-banner)` so it stays visible while user scrolls messages.

### Styling

CSS in existing `desktop-ui/src/styles/coding-todo.css`. New BEM classes:
- `coding-todo__plan-banner`
- `coding-todo__plan-banner-row`
- `coding-todo__plan-banner-title-edit`
- `coding-todo__plan-banner-actions`

Color tokens (from `ds-tokens.css`):
- Background: `var(--color-bg-elevated)`
- Border: `var(--color-accent-warm)` (matches InProgress status color from TodoWrite)
- Action button primary: `var(--color-accent-primary)`
- Action button danger (Cancel): `var(--color-fg-warning)`

Typography: `--fs-sm` for body rows; `--fs-md` for title row.

### Subscription

Consumes the existing `todoStore` (`useSyncExternalStore` hook). The store is updated by `useThreadEvents` reducer on `coding:todos_updated` events. No new store needed.

### Tests

- `PlanModeBanner.test.tsx`:
  - Renders 5 items from a fixture `CodingTodoView`.
  - Click on title opens inline edit; blur fires `coding_plan_user_edit` with the new title.
  - Click `[×]` fires `coding_plan_user_remove`.
  - Click `[Ratify & Execute]` then `[Confirm]` fires `coding_plan_ratify`.
  - Click `[Cancel Plan]` then `[Confirm]` fires `coding_plan_cancel`.
  - Banner not rendered when `plan_mode_state` is `None`.

---

## 12. New domain event variant

```rust
pub enum TodoEvent {
    // existing variants ...
    PlanCancelled {
        thread_id: String,
        plan_session_id: String,
        timestamp: jiff::Timestamp,
    },
}
```

Add to `crates/bus/src/domain_events.rs`. Mirror's `TodoSignalSource` ingests it for cognitive aggregation (cancellation rate per task type).

---

## 13. Invariants & error paths

| Invariant | Enforcement |
|---|---|
| Plan-mode policy is single-writer | `Arc<RwLock<>>` per session; policy swap holds write lock |
| `/plan` is no-op if already in plan mode | `coding_plan_enter` checks current variant; returns existing `CodingTodoView` |
| Ratify rejects unknown `plan_session_id` | Handler verifies policy variant + session id match before swap |
| User-edit / remove fail outside plan mode | `assert_plan_mode` returns `NotInPlanMode` error |
| Plan file deleted out-of-band | `Edit` recreates it via `OpenOptions::create(true)` |
| Subagent attempts write outside plan_file | Inherited `ClassifyHook` rejects with system-reminder |
| Plan-mode + non-pending status | Already enforced by `validate_plan_mode_pending_only` (shipped) |
| Concurrent `/plan` and `/plan-exit` | RwLock serializes; second waits |
| Untitled rename race (rename fires while user is editing) | Rename watcher uses `RwLock::write` on policy; UI re-fetches on `coding:plan_updated` |

### Error variants

```rust
pub enum CodingPlanError {
    NotCodingMode,
    NoPolicy,
    NotInPlanMode,
    PlanSessionMismatch { expected: String, got: String },
    PlanFileIoError(std::io::Error),
    InvalidItemsJson(serde_json::Error),
    ValidationFailed(CodingTodoError),
}
```

---

## 14. Testing strategy

### Unit tests

- `crates/approval/src/coding_policy.rs`:
  - `PlanMode` variant classifies `Edit` to plan-file as Allow.
  - `PlanMode` variant classifies `Edit` to other paths as Reject.
  - `PlanMode` variant classifies `Read` as Allow.
  - `PlanMode` variant classifies `coding_shell` as Reject.
- `crates/feature-coding-todo/src/util.rs`:
  - `kebab()` — 8 input/output cases (spaces, punctuation, case, length cap, leading/trailing dashes).
- `crates/app-core/src/handlers/coding_plan.rs`:
  - `compute_ratify_counts` — unchanged / edited / removed / mixed scenarios.

### Integration tests (in `crates/app-core/tests/`)

- `plan_mode_e2e.rs`:
  - **Happy path**: create coding session → `/plan` → LLM proposes 3 pending items → user ratifies → policy is `Default`, items have no `proposed_in_plan_session` tag, `TodoPlanRatified` published with correct counts.
  - **Edit flow**: enter → propose → user edits item 2 → final ratify counts: 2 ratified + 1 edited + 0 removed.
  - **Remove flow**: enter → propose 3 → user removes item 2 → final has 2 items, `TodoCancelled` emitted for item 2.
  - **Cancel flow**: enter → propose 3 → user cancels → all items soft-deleted, `TodoPlanCancelled` published, policy is `Default`.
  - **Cross-agent inheritance**: parent enters plan mode → spawns subagent → subagent's `Edit` to non-plan-file rejected with system-reminder.
  - **Untitled fallback + rename**: `/plan` fires before title arrives → file created with `untitled-<uuid8>` slug → `coding:thread_updated` arrives → file renamed → policy points at new path.
  - **Idempotent /plan**: enter twice in same session → second call no-ops, returns existing view.
  - **Plan file deleted out-of-band**: enter → user `rm`s plan file → LLM `Edit`s plan_file_path → file recreated.

### Frontend tests

- `PlanModeBanner.test.tsx` — listed in §11.
- `Composer.test.tsx` — `/plan` input is intercepted, doesn't reach send handler, fires `coding_plan_enter`.

### KCA gates

Add to `./scripts/run_kca_validation.sh`:
- Plan-ratification events present in event stream when ratify is invoked.
- Plan-mode `Edit` rejection emits a system-reminder visible in the next iteration's context.

---

## 15. Sequencing — two PRs

### PR 1 — Backend (3–4 days)

- §4 `CodingApprovalPolicy` enum refactor.
- §5 plan file lifecycle + `kebab()`.
- §6 `DynamicInjector` scaffold + `PlanModeInjector`.
- §7 `SubagentBuilder::inherit_plan_mode`.
- §8 `coding_plan_enter` / `coding_plan_cancel` handlers + Tauri commands.
- §9 four handler implementations (replace stubs).
- §10 Tauri command wiring + bindings regen.
- §12 `TodoEvent::PlanCancelled` variant.
- §13 invariants + error variants.
- §14 unit tests + integration tests.

PR 1 is mergeable on its own — backend works headlessly; users could trigger via Tauri devtools or by typing `/plan` (composer-side detection ships in PR 1 because it's tiny).

### PR 2 — Frontend (1–2 days)

- §11 `PlanModeBanner.tsx` full buildout (replace 15-line placeholder).
- Plan file open-in-editor link + `coding_plan_open_file` Tauri command.
- Confirmation inline UI for ratify/cancel.
- §11 frontend tests.
- Composer slash-command UX polish (toast, error handling).

PR 2 lights up the affordances. Dependencies on PR 1 only.

---

## 16. Dependencies & risks

| Dependency | Status | Impact |
|---|---|---|
| TodoWrite (Phase 2.1) | ✅ Shipped (commit `4de92d1ab`) | Foundation. All plan-mode plumbing is built on top. |
| `CodingApprovalPolicy` struct | ✅ Exists | Refactored to enum in this phase. |
| `RoutingContext.{plan_mode_active, plan_session_id}` | ✅ Exists, currently inert | Becomes live when policy is `PlanMode`. |
| `validate_plan_mode_pending_only` | ✅ Shipped | Enforces invariant unchanged. |
| `TodoEvent::PlanProposed` / `PlanRatified` | ✅ Shipped | Used as-is. |
| Phase 2.4 (hooks) | ⏳ Future | Reuses `DynamicInjector` scaffold. No blocking dependency. |
| Phase 1 (KlyntTracingProvider) | 🚧 In flight | Plan-mode events flow to wire log automatically when Phase 1 lands. |
| Phase 0.3 (mid-stream cancel) | 🚧 In flight | Independent. Cancel of a turn during plan mode still works (cancel kills the iteration; policy state is unchanged). |

### Risks

1. **Policy enum refactor break-the-world** — the struct is referenced from ~5 sites; the compiler walks them all. Mitigation: do the refactor as a pure mechanical change in a single commit before adding `PlanMode` semantics.
2. **Snapshot drift** — `plan_snapshots` is in-memory `DashMap`; lost on process restart mid-plan. Acceptable: ratify-counts default to "all edited" if snapshot is missing (graceful degradation, not a correctness issue).
3. **Filesystem race on rename watcher** — title arrives, file rename, in-flight `Edit` to old path fails. Mitigation: rename watcher takes the policy `write` lock for the duration of the rename; `Edit`s queue behind it.
4. **User edits during LLM iteration** — user edits an item in the banner while the LLM is mid-stream. Acceptable: each edit flushes the row; LLM sees updated list on next iteration via the existing `coding_todo` re-injection from `CodingTodoContextBuilder`.

---

## 17. Open questions

1. **`is_write_tool` for MCP tools** — the spec says "MCP tools tagged Destructive". Does the MCP bridge today expose `approval_class` per tool? Verify in `crates/mcp/`. If not, plan-mode treats unknown MCP tools as write-tools (conservative default).
2. **Slash-command discovery UX** — should `/` in the composer pop up a list of available slash commands? Not in scope for this phase, but Phase 2.5 (`/btw`) will want the same UI. Defer to that phase.
3. **Multiple plans per thread** — the design allows one active plan at a time. If a user wants to enter plan mode again *after* ratifying, they get a new `plan_session_id` and a new file (or reuses if slug collides). Should we surface old plans in some "Plans" sidebar? Defer; not in scope.
4. **Mobile/web client** — `/plan` is desktop-only for now. MCP exposure of plan mode is not in this phase.
5. **Plan template customization** — the stub markdown is hardcoded. Should `~/.klyntbot/plan-template.md` override it? Defer; YAGNI for v1.

---

## 18. Companion documents

- **TodoWrite spec (foundation):** `docs/superpowers/specs/2026-05-07-coding-todowrite-design.md`
- **TodoWrite implementation plan:** `docs/superpowers/plans/2026-05-07-coding-todowrite.md`
- **Comparative analysis (roadmap):** `docs/superpowers/notes/2026-05-07-long-running-task-comparative-analysis.md`
- **Permission gate (foundation):** `docs/superpowers/specs/2026-05-05-unified-permission-gate-design.md`
- **Implementation plan:** to be created via `superpowers:writing-plans` skill once this spec is approved.
