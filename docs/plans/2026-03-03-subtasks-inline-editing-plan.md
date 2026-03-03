# Sub-Tasks + Notion-Style Inline Editing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Display sub-tasks as expandable indented rows with progress indicators in the task table, and make all table cells inline-editable (Notion-style).

**Architecture:** Flat table with virtual nesting — parent and child tasks share the same `TaskRow` component, differentiated by a `depth` prop. Backend already has full sub-task hierarchy support via `parent_id` on the `actions` table; we bridge the gap by adding `parentId`/`subtaskCount`/`subtaskCompletedCount` to the Tauri response contract, then building inline editors and expand/collapse UI on the frontend.

**Tech Stack:** Rust (sqlx, Tauri v2), React 19, TypeScript, Tailwind v4 (CSS-driven tokens in `theme.css`), Radix UI primitives, lucide-react icons.

---

## Task 1: Storage — Add `count_completed_children` and `root_only` Filter

**Files:**
- Modify: `crates/storage/src/repos/action_repo.rs:15-27` (ActionFilter), `:680-687` (new method after count_children)
- Test: `crates/storage/src/repos/tests/action_repo_tests.rs`

**Step 1: Write the failing test for `count_completed_children`**

```rust
#[tokio::test]
async fn count_completed_children_returns_done_count() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = ActionRepo::new(pool.sqlite().clone());

    let parent = test_action("parent");
    repo.add(&parent).await.unwrap();

    let mut child1 = test_action_with_parent("child1", &parent.id);
    child1.status = "done".to_string();
    repo.add(&child1).await.unwrap();

    let child2 = test_action_with_parent("child2", &parent.id);
    repo.add(&child2).await.unwrap();

    let mut child3 = test_action_with_parent("child3", &parent.id);
    child3.status = "done".to_string();
    repo.add(&child3).await.unwrap();

    let count = repo.count_completed_children(&parent.id).await.unwrap();
    assert_eq!(count, 2);
}
```

Note: `test_action_with_parent` is a helper that creates an `ActionRow` with `parent_id = Some(id.to_string())`. If it doesn't exist yet, add it alongside the existing `test_action` helper in the test file.

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(count_completed_children)'`
Expected: FAIL — method `count_completed_children` does not exist

**Step 3: Implement `count_completed_children`**

In `crates/storage/src/repos/action_repo.rs`, after `count_children` (line 687):

```rust
/// Count immediate children with status = 'done'.
pub async fn count_completed_children(&self, parent_id: &str) -> Result<i64, StorageError> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM actions WHERE parent_id = ?1 AND status = 'done'"
    )
    .bind(parent_id)
    .fetch_one(&self.pool)
    .await?;
    Ok(row.0)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p storage -E 'test(count_completed_children)'`
Expected: PASS

**Step 5: Write the failing test for `root_only` filter**

```rust
#[tokio::test]
async fn list_with_root_only_excludes_children() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = ActionRepo::new(pool.sqlite().clone());

    let parent = test_action("parent-task");
    repo.add(&parent).await.unwrap();

    let child = test_action_with_parent("child-task", &parent.id);
    repo.add(&child).await.unwrap();

    let root_only = test_action("root-only-task");
    repo.add(&root_only).await.unwrap();

    let filter = ActionFilter { root_only: true, ..Default::default() };
    let results = repo.list(&filter).await.unwrap();

    assert!(results.iter().all(|r| r.parent_id.is_none()));
    assert_eq!(results.len(), 2); // parent + root_only, not child
}
```

**Step 6: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(list_with_root_only)'`
Expected: FAIL — `root_only` field does not exist on `ActionFilter`

**Step 7: Add `root_only` field to `ActionFilter` and wire it into the list query**

In `crates/storage/src/repos/action_repo.rs:15-27`, add to `ActionFilter`:

```rust
pub root_only: bool,
```

Then find the `list()` method's WHERE clause builder and add:

```rust
if filter.root_only {
    conditions.push("parent_id IS NULL");
}
```

Also update `lib.rs` re-export if ActionFilter is exported there.

**Step 8: Run tests to verify**

Run: `cargo nextest run -p storage -E 'test(list_with_root_only)' && cargo nextest run -p storage -E 'test(count_completed_children)'`
Expected: Both PASS

**Step 9: Run full storage tests to ensure no regressions**

Run: `cargo nextest run -p storage`
Expected: All pass

**Step 10: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): add count_completed_children and root_only filter to ActionRepo"
```

---

## Task 2: Desktop Contract — Extend `TaskResponse` and `TaskCreateParams`

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs:10-22` (TaskResponse), `:26-33` (TaskCreateParams)
- Modify: `crates/desktop/src/commands/tasks.rs:19-33` (action_to_task), `:99-119` (task_list), `:214-261` (task_create)
- Modify: `crates/desktop/src/main.rs:105-145` (generate_handler)

**Step 1: Add fields to `TaskResponse`**

In `crates/desktop-shared/src/commands.rs`, add to `TaskResponse` (after line 21):

```rust
pub parent_id: Option<String>,
pub subtask_count: u32,
pub subtask_completed_count: u32,
```

**Step 2: Add `parent_id` to `TaskCreateParams`**

In `crates/desktop-shared/src/commands.rs`, add to `TaskCreateParams` (after line 33):

```rust
pub parent_id: Option<String>,
```

**Step 3: Update `action_to_task` to include new fields (temporarily hardcoded to 0)**

In `crates/desktop/src/commands/tasks.rs:19-33`, update `action_to_task` to accept counts as parameters:

```rust
pub fn action_to_task(row: &ActionRow, subtask_count: u32, subtask_completed_count: u32) -> TaskResponse {
    TaskResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        completed: row.status == "done",
        priority: priority_label(row.priority),
        status: row.status.clone(),
        due_date: row.due_date.map(|d| d.format("%b %-d").to_string()),
        tags: row.tags.clone(),
        project_id: row.project_id.clone(),
        area_id: row.area_id.clone(),
        objective_id: row.key_result_id.clone(),
        description: row.description.clone(),
        parent_id: row.parent_id.clone(),
        subtask_count,
        subtask_completed_count,
    }
}
```

**Step 4: Fix all call sites of `action_to_task`**

Search all call sites of `action_to_task` (task_list, task_create, task_update, task_toggle_complete, today_tasks adapter, agent_status focus_task). For now pass `0, 0` as subtask counts to compile. We'll add real counts in the next step.

**Step 5: Update `task_list` to filter root-only and populate subtask counts**

In `crates/desktop/src/commands/tasks.rs`, update `task_list`:

```rust
#[tauri::command]
pub async fn task_list(
    state: State<'_, AppCore>,
    area_id: Option<String>,
    project_id: Option<String>,
    status: Option<String>,
) -> Result<Vec<TaskResponse>, ApiError> {
    let filter = ActionFilter {
        area_id,
        project_id,
        status,
        root_only: true,
        ..Default::default()
    };
    let rows = state.repos.actions.list(&filter).await.map_err(super::map_storage_err)?;

    let mut results = Vec::with_capacity(rows.len());
    for row in &rows {
        let (total, completed) = tokio::try_join!(
            state.repos.actions.count_children(&row.id),
            state.repos.actions.count_completed_children(&row.id),
        ).map_err(super::map_storage_err)?;
        results.push(action_to_task(row, total as u32, completed as u32));
    }
    Ok(results)
}
```

**Step 6: Add `task_list_children` command**

In `crates/desktop/src/commands/tasks.rs`, add after `task_list`:

```rust
#[tauri::command]
pub async fn task_list_children(
    state: State<'_, AppCore>,
    parent_id: String,
) -> Result<Vec<TaskResponse>, ApiError> {
    let rows = state.repos.actions.get_children(&parent_id).await.map_err(super::map_storage_err)?;

    let mut results = Vec::with_capacity(rows.len());
    for row in &rows {
        let (total, completed) = tokio::try_join!(
            state.repos.actions.count_children(&row.id),
            state.repos.actions.count_completed_children(&row.id),
        ).map_err(super::map_storage_err)?;
        results.push(action_to_task(row, total as u32, completed as u32));
    }
    Ok(results)
}
```

**Step 7: Update `task_create` to support `parent_id`**

In `crates/desktop/src/commands/tasks.rs:214-261`, change `parent_id: None` to `parent_id: params.parent_id.clone()` at line 230. Also, when parent_id is set, inherit area_id from parent if not explicitly provided:

```rust
let area_id = if let Some(ref pid) = params.parent_id {
    if params.area_id.is_none() {
        let parent = state.repos.actions.get_or_err(pid).await.map_err(super::map_storage_err)?;
        parent.area_id
    } else {
        params.area_id.unwrap_or_else(|| "default".to_string())
    }
} else {
    params.area_id.unwrap_or_else(|| "default".to_string())
};
```

**Step 8: Register `task_list_children` in `main.rs`**

In `crates/desktop/src/main.rs:112`, add after `task_toggle_complete`:

```rust
commands::tasks::task_list_children,
```

**Step 9: Build and verify compilation**

Run: `cargo build -p desktop`
Expected: Compiles with 0 errors

**Step 10: Run clippy**

Run: `cargo clippy -p desktop -p desktop-shared --all-targets`
Expected: 0 warnings

**Step 11: Commit**

```bash
git add crates/desktop-shared/ crates/desktop/
git commit -m "feat(desktop): add subtask fields to TaskResponse, task_list_children command"
```

---

## Task 3: Frontend Types + Hooks

**Files:**
- Modify: `desktop-ui/src/lib/types.ts:6-18` (Task interface), `:381-392` (TaskUpdateParams)
- Create: `desktop-ui/src/hooks/useSubtasks.ts`

**Step 1: Update `Task` interface**

In `desktop-ui/src/lib/types.ts:6-18`, add after `description`:

```typescript
export interface Task {
  id: string;
  title: string;
  completed: boolean;
  priority: string | null;
  status: string;
  dueDate: string | null;
  tags: string[];
  projectId: string | null;
  areaId: string;
  objectiveId?: string;
  description?: string;
  parentId: string | null;
  subtaskCount: number;
  subtaskCompletedCount: number;
}
```

**Step 2: Add `TaskCreateParams` interface**

In `desktop-ui/src/lib/types.ts`, after the existing `TaskUpdateParams`:

```typescript
export interface TaskCreateParams {
  title: string;
  areaId?: string;
  projectId?: string;
  priority?: number;
  dueDate?: string;
  tags?: string[];
  parentId?: string;
}
```

**Step 3: Create `useSubtasks` hook**

Create `desktop-ui/src/hooks/useSubtasks.ts`:

```typescript
import { useState, useCallback } from 'react';
import { ipc } from './useIpc';
import type { Task } from '../lib/types';

export function useSubtasks() {
  const [childrenCache, setChildrenCache] = useState<Map<string, Task[]>>(new Map());
  const [loadingChildren, setLoadingChildren] = useState<Set<string>>(new Set());
  const [expandedTasks, setExpandedTasks] = useState<Set<string>>(new Set());

  const toggleExpand = useCallback((taskId: string) => {
    setExpandedTasks(prev => {
      const next = new Set(prev);
      if (next.has(taskId)) next.delete(taskId);
      else next.add(taskId);
      return next;
    });
  }, []);

  const fetchChildren = useCallback(async (parentId: string) => {
    if (loadingChildren.has(parentId)) return;

    setLoadingChildren(prev => new Set(prev).add(parentId));
    try {
      const children = await ipc<Task[]>('task_list_children', { parentId });
      setChildrenCache(prev => new Map(prev).set(parentId, children));
    } finally {
      setLoadingChildren(prev => {
        const next = new Set(prev);
        next.delete(parentId);
        return next;
      });
    }
  }, [loadingChildren]);

  const invalidateCache = useCallback(() => {
    setChildrenCache(new Map());
  }, []);

  return {
    expandedTasks,
    childrenCache,
    loadingChildren,
    toggleExpand,
    fetchChildren,
    invalidateCache,
  };
}
```

**Step 4: Verify types compile**

Run: `cd desktop-ui && npx tsc --noEmit`
Expected: 0 errors (or only pre-existing ones unrelated to our changes)

**Step 5: Commit**

```bash
git add desktop-ui/src/lib/types.ts desktop-ui/src/hooks/useSubtasks.ts
git commit -m "feat(desktop-ui): add subtask fields to Task type and useSubtasks hook"
```

---

## Task 4: Inline Editor Components

**Files:**
- Create: `desktop-ui/src/components/tasks/editors/InlineSelect.tsx`
- Create: `desktop-ui/src/components/tasks/editors/InlineTagsEditor.tsx`
- Create: `desktop-ui/src/components/tasks/editors/InlineDatePicker.tsx`
- Create: `desktop-ui/src/components/tasks/editors/InlineTextEditor.tsx`

**Step 1: Create `InlineSelect` — reusable click-to-edit dropdown**

Create `desktop-ui/src/components/tasks/editors/InlineSelect.tsx`:

```tsx
import { useState, useRef, useEffect } from 'react';

interface Option {
  value: string | null;
  label: string;
  className?: string;
}

interface InlineSelectProps {
  value: string | null;
  options: Option[];
  onSelect: (value: string | null) => void;
  renderDisplay: (value: string | null) => React.ReactNode;
  className?: string;
}

export function InlineSelect({ value, options, onSelect, renderDisplay, className }: InlineSelectProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  return (
    <div ref={ref} className={`relative ${className ?? ''}`}>
      <button
        onClick={(e) => { e.stopPropagation(); setOpen(!open); }}
        className="w-full text-left rounded px-1 -mx-1 hover:bg-surface-higher transition-colors"
      >
        {renderDisplay(value)}
      </button>
      {open && (
        <div className="absolute z-50 top-full left-0 mt-1 min-w-[120px] bg-surface-highest border border-border rounded-lg shadow-lg py-1">
          {options.map(opt => (
            <button
              key={opt.value ?? '__none'}
              onClick={(e) => { e.stopPropagation(); onSelect(opt.value); setOpen(false); }}
              className={`w-full text-left px-3 py-1.5 text-[12px] font-light hover:bg-surface-base transition-colors ${
                value === opt.value ? 'text-brand' : 'text-secondary'
              } ${opt.className ?? ''}`}
            >
              {opt.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
```

**Step 2: Create `InlineTextEditor` — click-to-edit text input**

Create `desktop-ui/src/components/tasks/editors/InlineTextEditor.tsx`:

```tsx
import { useState, useCallback } from 'react';

interface InlineTextEditorProps {
  value: string;
  onSave: (value: string) => void;
  className?: string;
  placeholder?: string;
}

export function InlineTextEditor({ value, onSave, className, placeholder }: InlineTextEditorProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');

  const startEdit = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setDraft(value);
    setEditing(true);
  }, [value]);

  const save = useCallback(() => {
    if (draft.trim() && draft !== value) onSave(draft.trim());
    setEditing(false);
  }, [draft, value, onSave]);

  if (editing) {
    return (
      <input
        autoFocus
        value={draft}
        onChange={e => setDraft(e.target.value)}
        onKeyDown={e => {
          if (e.key === 'Enter') save();
          if (e.key === 'Escape') setEditing(false);
        }}
        onBlur={save}
        onClick={e => e.stopPropagation()}
        placeholder={placeholder}
        className={`bg-transparent border-b border-brand outline-none w-full ${className ?? 'text-[13px] font-light text-primary'}`}
      />
    );
  }

  return (
    <span
      onClick={startEdit}
      className={`cursor-text rounded px-1 -mx-1 hover:bg-surface-higher transition-colors ${className ?? 'text-[13px] font-light text-secondary'}`}
    >
      {value || <span className="text-dim">{placeholder ?? '—'}</span>}
    </span>
  );
}
```

**Step 3: Create `InlineTagsEditor` — multi-tag editor**

Create `desktop-ui/src/components/tasks/editors/InlineTagsEditor.tsx`:

```tsx
import { useState, useRef, useEffect } from 'react';
import { X } from 'lucide-react';
import { Badge } from '../../ui/Badge';

interface InlineTagsEditorProps {
  tags: string[];
  onSave: (tags: string[]) => void;
  suggestions?: string[];
}

export function InlineTagsEditor({ tags, onSave, suggestions = [] }: InlineTagsEditorProps) {
  const [open, setOpen] = useState(false);
  const [input, setInput] = useState('');
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  const addTag = (tag: string) => {
    const trimmed = tag.trim().toLowerCase();
    if (trimmed && !tags.includes(trimmed)) {
      onSave([...tags, trimmed]);
    }
    setInput('');
  };

  const removeTag = (tag: string) => {
    onSave(tags.filter(t => t !== tag));
  };

  const filtered = suggestions.filter(s => !tags.includes(s) && s.includes(input.toLowerCase()));

  return (
    <div ref={ref} className="relative">
      <div
        onClick={(e) => { e.stopPropagation(); setOpen(true); }}
        className="flex items-center gap-1 flex-wrap cursor-text rounded px-1 -mx-1 min-h-[24px] hover:bg-surface-higher transition-colors"
      >
        {tags.map(tag => (
          <span key={tag} className="inline-flex items-center gap-0.5">
            <Badge variant="tag" value={tag} />
            {open && (
              <button onClick={(e) => { e.stopPropagation(); removeTag(tag); }} className="text-dim hover:text-destructive">
                <X className="w-2.5 h-2.5" />
              </button>
            )}
          </span>
        ))}
        {tags.length === 0 && !open && <span className="text-[11px] text-dim">—</span>}
      </div>
      {open && (
        <div className="absolute z-50 top-full left-0 mt-1 min-w-[180px] bg-surface-highest border border-border rounded-lg shadow-lg p-2">
          <input
            autoFocus
            value={input}
            onChange={e => setInput(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter' && input.trim()) { addTag(input); }
              if (e.key === 'Escape') setOpen(false);
            }}
            onClick={e => e.stopPropagation()}
            placeholder="Add tag..."
            className="w-full bg-transparent text-[12px] font-light text-primary outline-none placeholder:text-dim mb-1"
          />
          {filtered.length > 0 && (
            <div className="border-t border-border pt-1 mt-1">
              {filtered.slice(0, 5).map(s => (
                <button
                  key={s}
                  onClick={(e) => { e.stopPropagation(); addTag(s); }}
                  className="w-full text-left px-2 py-1 text-[11px] font-light text-muted hover:bg-surface-base rounded"
                >
                  {s}
                </button>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
```

**Step 4: Create `InlineDatePicker` — date editor**

Create `desktop-ui/src/components/tasks/editors/InlineDatePicker.tsx`:

```tsx
import { useState, useRef, useEffect } from 'react';

interface InlineDatePickerProps {
  value: string | null;
  onSave: (value: string | null) => void;
}

export function InlineDatePicker({ value, onSave }: InlineDatePickerProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <button
        onClick={(e) => { e.stopPropagation(); setOpen(!open); }}
        className="text-[12px] text-muted font-light rounded px-1 -mx-1 hover:bg-surface-higher transition-colors"
      >
        {value || <span className="text-dim">—</span>}
      </button>
      {open && (
        <div className="absolute z-50 top-full left-0 mt-1 bg-surface-highest border border-border rounded-lg shadow-lg p-2">
          <input
            type="date"
            autoFocus
            onChange={(e) => {
              onSave(e.target.value || null);
              setOpen(false);
            }}
            onClick={e => e.stopPropagation()}
            className="bg-transparent text-[12px] font-light text-primary outline-none"
          />
          {value && (
            <button
              onClick={(e) => { e.stopPropagation(); onSave(null); setOpen(false); }}
              className="w-full text-left mt-1 px-2 py-1 text-[11px] text-destructive hover:bg-surface-base rounded"
            >
              Clear date
            </button>
          )}
        </div>
      )}
    </div>
  );
}
```

**Step 5: Verify components compile**

Run: `cd desktop-ui && npx tsc --noEmit`
Expected: 0 errors

**Step 6: Commit**

```bash
git add desktop-ui/src/components/tasks/editors/
git commit -m "feat(desktop-ui): add inline editor components (select, text, tags, date)"
```

---

## Task 5: SubtaskProgress Component

**Files:**
- Create: `desktop-ui/src/components/tasks/SubtaskProgress.tsx`

**Step 1: Create `SubtaskProgress` component**

Create `desktop-ui/src/components/tasks/SubtaskProgress.tsx`:

```tsx
interface SubtaskProgressProps {
  total: number;
  completed: number;
}

export function SubtaskProgress({ total, completed }: SubtaskProgressProps) {
  if (total === 0) return null;

  const pct = Math.round((completed / total) * 100);

  return (
    <div className="inline-flex items-center gap-1.5 ml-2 flex-shrink-0">
      <div className="w-10 h-1 rounded-full bg-surface-higher overflow-hidden">
        <div
          className="h-full rounded-full bg-brand transition-all"
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-[11px] text-muted font-light tabular-nums">
        {completed}/{total}
      </span>
    </div>
  );
}
```

**Step 2: Verify it compiles**

Run: `cd desktop-ui && npx tsc --noEmit`
Expected: 0 errors

**Step 3: Commit**

```bash
git add desktop-ui/src/components/tasks/SubtaskProgress.tsx
git commit -m "feat(desktop-ui): add SubtaskProgress component"
```

---

## Task 6: AddSubtaskRow Component

**Files:**
- Create: `desktop-ui/src/components/tasks/AddSubtaskRow.tsx`

**Step 1: Create `AddSubtaskRow` component**

Create `desktop-ui/src/components/tasks/AddSubtaskRow.tsx`:

```tsx
import { useState, useCallback } from 'react';
import { Plus } from 'lucide-react';

interface AddSubtaskRowProps {
  parentId: string;
  isLast: boolean;
  showArea: boolean;
  onCreate: (parentId: string, title: string) => void;
}

export function AddSubtaskRow({ parentId, isLast, showArea, onCreate }: AddSubtaskRowProps) {
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState('');

  const colCount = showArea ? 8 : 7;

  const save = useCallback(() => {
    if (title.trim()) {
      onCreate(parentId, title.trim());
      setTitle('');
    }
    setEditing(false);
  }, [title, parentId, onCreate]);

  return (
    <tr className="border-b border-border-subtle last:border-b-0">
      <td className="px-5 py-2 w-9" />
      <td colSpan={colCount - 1} className="px-5 py-2" onClick={e => e.stopPropagation()}>
        <div className="flex items-center" style={{ paddingLeft: 32 }}>
          {/* Tree connector */}
          <div className="flex items-center mr-2 text-border">
            <span className="text-[12px]">{isLast ? '└─' : '├─'}</span>
          </div>
          {editing ? (
            <input
              autoFocus
              value={title}
              onChange={e => setTitle(e.target.value)}
              onKeyDown={e => {
                if (e.key === 'Enter') save();
                if (e.key === 'Escape') { setEditing(false); setTitle(''); }
              }}
              onBlur={save}
              placeholder="Subtask title..."
              className="bg-transparent text-[12px] font-light text-primary outline-none placeholder:text-dim flex-1"
            />
          ) : (
            <button
              onClick={() => setEditing(true)}
              className="flex items-center gap-1 text-[12px] font-light text-dim hover:text-muted transition-colors"
            >
              <Plus className="w-3 h-3" />
              Add subtask
            </button>
          )}
        </div>
      </td>
    </tr>
  );
}
```

**Step 2: Verify it compiles**

Run: `cd desktop-ui && npx tsc --noEmit`
Expected: 0 errors

**Step 3: Commit**

```bash
git add desktop-ui/src/components/tasks/AddSubtaskRow.tsx
git commit -m "feat(desktop-ui): add AddSubtaskRow component"
```

---

## Task 7: Refactor TaskRow — Inline Editing + Depth Support

This is the largest frontend task. We refactor `TaskRow` to support:
- `depth` prop (0 = root, 1 = sub-task)
- Tree connector rendering for depth > 0
- SubtaskProgress display for depth 0 with subtasks
- Expand/collapse chevron
- All cells become inline-editable

**Files:**
- Modify: `desktop-ui/src/components/tasks/TaskRow.tsx`

**Step 1: Rewrite `TaskRow` with full inline editing and depth support**

The full rewrite of `TaskRow.tsx`. Key changes:
- Add `depth`, `isLast`, `isExpanded`, `onToggleExpand` props
- Replace double-click title edit with `InlineTextEditor` (single-click)
- Replace priority cycling with `InlineSelect` (PrioritySelect)
- Add `InlineSelect` for status
- Add `InlineSelect` for project and area
- Add `InlineDatePicker` for due date
- Add `InlineTagsEditor` for tags
- Add `SubtaskProgress` after title for root tasks with subtasks
- Add chevron before checkbox for expandable tasks
- Add tree connector lines for depth > 0
- Prevent row click navigation when clicking an editable cell

New props interface:

```typescript
interface TaskRowProps {
  task: Task;
  depth?: number;
  isLast?: boolean;
  isExpanded?: boolean;
  project?: Project;
  area?: Area;
  projects: Project[];
  areas: Area[];
  isCompleted: boolean;
  showArea: boolean;
  tagSuggestions?: string[];
  onToggle: () => void;
  onToggleExpand?: () => void;
  onUpdate: (params: TaskUpdateParams) => void;
}
```

Replace the old `onUpdatePriority` and `onRename` with a single `onUpdate` callback that accepts `TaskUpdateParams`. This unifies all inline edits through one path.

**Implementation notes:**
- The chevron only renders when `task.subtaskCount > 0`. Click calls `onToggleExpand`.
- Tree connectors use `├─` for non-last children, `└─` for the last child, rendered as text with `text-border` color.
- Row navigation (`navigate('/task/${task.id}')`) only fires when clicking empty space — not on any interactive cell. Wrap the `onClick` handler to check `e.target` is the `<tr>` or a non-interactive `<td>`.
- All `InlineSelect` dropdowns use `e.stopPropagation()` internally to prevent row navigation.

**Step 2: Verify it compiles**

Run: `cd desktop-ui && npx tsc --noEmit`
Expected: 0 errors (may have warnings about unused old props, fix them)

**Step 3: Commit**

```bash
git add desktop-ui/src/components/tasks/TaskRow.tsx
git commit -m "feat(desktop-ui): refactor TaskRow with inline editing and depth support"
```

---

## Task 8: Refactor TaskTable — Flat Display Array + Expand/Collapse

**Files:**
- Modify: `desktop-ui/src/components/tasks/TaskTable.tsx`

**Step 1: Update `TaskTableProps` to accept subtask state**

Add new props for subtask management:

```typescript
interface TaskTableProps {
  tasks: Task[];
  projectMap: Map<string, Project>;
  projects: Project[];
  objectives: Objective[];
  areaMap: Map<string, Area>;
  areas: Area[];
  activeTab: Tab;
  completedTasks: Set<string>;
  collapsedProjects: Set<string>;
  expandedTasks: Set<string>;
  childrenCache: Map<string, Task[]>;
  loadingChildren: Set<string>;
  tagSuggestions: string[];
  onToggleTask: (taskId: string) => void;
  onToggleProject: (projectId: string) => void;
  onToggleExpand: (taskId: string) => void;
  onUpdate: (params: TaskUpdateParams) => void;
  onCreateSubtask: (parentId: string, title: string) => void;
}
```

**Step 2: Build flat display array with expanded children**

Inside the `ProjectGroup` and `UnassignedGroup` components, when rendering task rows, build a flat list that interleaves parent rows with their expanded children:

```typescript
// For each task in the group:
// 1. Render the task (depth=0)
// 2. If expanded and children loaded, render each child (depth=1)
// 3. If expanded, render AddSubtaskRow
```

**Step 3: Update `TaskRow` usage to pass new props**

Pass `depth`, `isLast`, `isExpanded`, `onToggleExpand`, `onUpdate`, `projects`, `areas`, `tagSuggestions` to every `TaskRow`.

**Step 4: Add loading indicator for expanding tasks**

When `loadingChildren.has(taskId)` is true, show a small spinner or "Loading..." text in place of children.

**Step 5: Verify it compiles**

Run: `cd desktop-ui && npx tsc --noEmit`
Expected: 0 errors

**Step 6: Commit**

```bash
git add desktop-ui/src/components/tasks/TaskTable.tsx
git commit -m "feat(desktop-ui): refactor TaskTable with subtask expansion and flat display array"
```

---

## Task 9: Wire Everything in MainApp

**Files:**
- Modify: `desktop-ui/src/components/views/MainApp.tsx`

**Step 1: Add `useSubtasks` hook**

Import and use the `useSubtasks` hook:

```typescript
const { expandedTasks, childrenCache, loadingChildren, toggleExpand, fetchChildren, invalidateCache } = useSubtasks();
```

**Step 2: Auto-fetch children on expand**

Add an effect that fetches children when a task is expanded and not cached:

```typescript
useEffect(() => {
  for (const taskId of expandedTasks) {
    if (!childrenCache.has(taskId) && !loadingChildren.has(taskId)) {
      fetchChildren(taskId);
    }
  }
}, [expandedTasks, childrenCache, loadingChildren, fetchChildren]);
```

**Step 3: Invalidate subtask cache on entity updates**

In the `useEvent('entity:updated', ...)` handler, add `invalidateCache()` when `kind === 'task'`.

**Step 4: Add `handleCreateSubtask` callback**

```typescript
const handleCreateSubtask = useCallback(async (parentId: string, title: string) => {
  await createTask.mutate({ title, parentId });
}, [createTask]);
```

Update `createTask` mutation type to use `TaskCreateParams` instead of `{ title: string }`.

**Step 5: Add unified `handleUpdateTask` callback**

Replace `handleUpdatePriority` and `handleRenameTask` with a single:

```typescript
const handleUpdateTask = useCallback(async (params: TaskUpdateParams) => {
  await updateTask.mutate(params);
}, [updateTask]);
```

**Step 6: Collect tag suggestions from all tasks**

```typescript
const tagSuggestions = useMemo(() => {
  const all = new Set<string>();
  tasks.forEach(t => t.tags.forEach(tag => all.add(tag)));
  return Array.from(all).sort();
}, [tasks]);
```

**Step 7: Pass new props to `TaskTable`**

```tsx
<TaskTable
  tasks={filteredTasks}
  projectMap={projectMap}
  projects={projects}
  objectives={objectives}
  areaMap={areaMap}
  areas={areas}
  activeTab={activeTab}
  completedTasks={completedTasks}
  collapsedProjects={collapsedProjects}
  expandedTasks={expandedTasks}
  childrenCache={childrenCache}
  loadingChildren={loadingChildren}
  tagSuggestions={tagSuggestions}
  onToggleTask={handleToggleTask}
  onToggleProject={toggleProject}
  onToggleExpand={toggleExpand}
  onUpdate={handleUpdateTask}
  onCreateSubtask={handleCreateSubtask}
/>
```

**Step 8: Verify the full app compiles and runs**

Run: `cd desktop-ui && npx tsc --noEmit && bun run dev`
Expected: Compiles, dev server starts

**Step 9: Commit**

```bash
git add desktop-ui/src/components/views/MainApp.tsx
git commit -m "feat(desktop-ui): wire subtask state and inline editing in MainApp"
```

---

## Task 10: Full Build Verification + Clippy

**Step 1: Run Rust build + clippy**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets --all-features`
Expected: 0 errors, 0 warnings

**Step 2: Run Rust tests**

Run: `cargo nextest run --workspace`
Expected: All pass

**Step 3: Run frontend type check + build**

Run: `cd desktop-ui && npx tsc --noEmit && bun run build`
Expected: 0 errors, build succeeds

**Step 4: Manual smoke test**

- Start dev server: `cd desktop-ui && bun run dev`
- Verify task table renders with existing tasks
- Check that tasks with no subtasks look identical to before (no chevron, no progress)
- If you have tasks with subtasks in the database, verify the progress indicator shows

**Step 5: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "fix: address build/lint issues from subtask + inline editing feature"
```
