# Typed IPC, TanStack Query, & Push Invalidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace string-keyed Tauri IPC with generated typed bindings, migrate the FE cache from custom hooks to `@tanstack/react-query`, and implement push-based cache invalidation driven by typed Rust events.

**Architecture:** Three phases. Phase 1 adds `specta` + `tauri-specta` codegen producing a `bindings.ts` file plus a Tauri/HTTP dispatch adapter. Phase 2 replaces the homegrown `useQuery`/`useMutation` cache with `@tanstack/react-query` wrappers that consume the typed bindings. Phase 3 emits typed `EntityUpdate` events from Rust mutation handlers, and a single `EntitySyncProvider` on the FE drives all cache invalidation through a registry.

**Tech Stack:** Rust (Tauri 2, specta 2.x, tauri-specta 2.x, serde), TypeScript (React 19, Vite, `@tanstack/react-query` v5, Biome 2.0, Vitest).

**Pre-release context:** No users, no migrations needed. Big-bang cutover is acceptable. `cargo nextest` for Rust tests; `bun run test` for Vitest.

---

## Phase 1 — Typed bindings

### Task 1: Add specta + tauri-specta dependencies

**Files:**
- Modify: `crates/desktop/Cargo.toml`
- Modify: `crates/desktop-shared/Cargo.toml`
- Modify: `Cargo.toml` (workspace root — add to `[workspace.dependencies]`)

- [ ] **Step 1: Add to workspace deps**

In `Cargo.toml` (root), under `[workspace.dependencies]`, add:
```toml
specta = { version = "=2.0.0-rc.22", features = ["derive", "serde", "serde_json", "chrono", "uuid"] }
specta-typescript = "0.0.9"
tauri-specta = { version = "=2.0.0-rc.21", features = ["derive", "typescript"] }
```

- [ ] **Step 2: Wire into `desktop-shared`**

In `crates/desktop-shared/Cargo.toml` under `[dependencies]`:
```toml
specta = { workspace = true }
```

- [ ] **Step 3: Wire into `desktop`**

In `crates/desktop/Cargo.toml` under `[dependencies]`:
```toml
specta = { workspace = true }
specta-typescript = { workspace = true }
tauri-specta = { workspace = true }
```

Under `[build-dependencies]`:
```toml
tauri-build = { version = "2", features = [] }
```

- [ ] **Step 4: Verify the workspace builds**

Run: `cargo build --workspace`
Expected: success, no new warnings.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/desktop/Cargo.toml crates/desktop-shared/Cargo.toml Cargo.lock
git commit -m "build: add specta + tauri-specta dependencies"
```

---

### Task 2: Scaffold codegen with one proof-of-concept command

**Files:**
- Create: `crates/desktop/build.rs` (if missing; otherwise modify)
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/commands/settings.rs` (pick one small command as proof-of-concept; use `get_config` or similar)
- Create: `desktop-ui/src/shared/ipc/bindings.ts` (generated, will be overwritten)

- [ ] **Step 1: Write a Rust test asserting codegen output exists**

Create `crates/desktop/tests/codegen_smoke.rs`:
```rust
use std::path::PathBuf;

#[test]
fn bindings_file_is_generated() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../desktop-ui/src/shared/ipc/bindings.ts");
    assert!(path.exists(), "bindings.ts should be generated at {}", path.display());
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("export const commands"), "should export commands object");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p desktop --test codegen_smoke`
Expected: FAIL with "bindings.ts should be generated at …".

- [ ] **Step 3: Pick a proof-of-concept command**

Open `crates/desktop/src/commands/settings.rs`. Find a simple command (prefer a read with no args like `get_app_version` or similar). Add `#[specta::specta]` to it:
```rust
#[tauri::command]
#[specta::specta]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
```

If the return type is a struct, also add `#[derive(specta::Type)]` to that struct.

- [ ] **Step 4: Wire tauri-specta in `lib.rs`**

In `crates/desktop/src/lib.rs`, near the top add:
```rust
use tauri_specta::{collect_commands, collect_events};
```

Inside `run()` (or wherever the builder is constructed), before `tauri::Builder`:
```rust
let builder = tauri_specta::Builder::<tauri::Wry>::new()
    .commands(collect_commands![
        commands::settings::get_app_version,
    ])
    .events(collect_events![]);

#[cfg(debug_assertions)]
builder
    .export(
        specta_typescript::Typescript::default()
            .bigint(specta_typescript::BigIntExportBehavior::Number)
            .formatter(specta_typescript::formatter::prettier),
        "../../desktop-ui/src/shared/ipc/bindings.ts",
    )
    .expect("Failed to export typescript bindings");
```

Then chain `.invoke_handler(builder.invoke_handler())` on the Tauri builder and `.setup(move |app| { builder.mount_events(app); Ok(()) })`.

- [ ] **Step 5: Build desktop crate to trigger codegen**

Run: `cargo build -p desktop`
Expected: success. Inspect `desktop-ui/src/shared/ipc/bindings.ts` — should contain `commands.getAppVersion` (or similar).

- [ ] **Step 6: Run the smoke test**

Run: `cargo nextest run -p desktop --test codegen_smoke`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/src/lib.rs crates/desktop/src/commands/settings.rs crates/desktop/tests/codegen_smoke.rs desktop-ui/src/shared/ipc/bindings.ts
git commit -m "feat(desktop): scaffold tauri-specta codegen with proof-of-concept command"
```

---

### Task 3: Create dispatch adapter (Tauri invoke vs dev-server fetch)

**Files:**
- Create: `desktop-ui/src/shared/ipc/dispatch.ts`
- Create: `desktop-ui/src/shared/ipc/dispatch.test.ts`
- Create: `desktop-ui/src/shared/ipc/index.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// desktop-ui/src/shared/ipc/dispatch.test.ts
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { dispatch } from "./dispatch";

describe("dispatch adapter", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("uses Tauri invoke when __TAURI_INTERNALS__ is present", async () => {
    const invokeMock = vi.fn().mockResolvedValue({ ok: true });
    vi.stubGlobal("__TAURI_INTERNALS__", { invoke: invokeMock });
    vi.stubGlobal("window", { __TAURI_INTERNALS__: { invoke: invokeMock } });

    const result = await dispatch<{ ok: boolean }>("get_app_version", {});
    expect(result).toEqual({ ok: true });
    expect(invokeMock).toHaveBeenCalledWith("get_app_version", {});
  });

  it("falls back to fetch in browser dev mode", async () => {
    vi.stubGlobal("window", {});
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ version: "0.1.0" }),
    });
    vi.stubGlobal("fetch", fetchMock);

    const result = await dispatch<{ version: string }>("get_app_version", {});
    expect(result).toEqual({ version: "0.1.0" });
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/get_app_version",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("normalizes fetch errors to ApiError shape", async () => {
    vi.stubGlobal("window", {});
    const fetchMock = vi.fn().mockResolvedValue({
      ok: false,
      statusText: "bad",
      json: async () => ({ code: "E_BAD", message: "nope" }),
    });
    vi.stubGlobal("fetch", fetchMock);

    await expect(dispatch("x", {})).rejects.toMatchObject({ code: "E_BAD", message: "nope" });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd desktop-ui && bun run test dispatch`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement dispatch**

```typescript
// desktop-ui/src/shared/ipc/dispatch.ts
import { invoke } from "@tauri-apps/api/core";

export type DispatchFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

const isTauriRuntime = (): boolean =>
  typeof window !== "undefined" &&
  typeof (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== "undefined";

export const dispatch: DispatchFn = async <T>(cmd: string, args: Record<string, unknown> = {}) => {
  if (isTauriRuntime()) {
    return invoke<T>(cmd, args);
  }
  const res = await fetch(`/api/${cmd}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ code: "NETWORK", message: res.statusText }));
    throw err;
  }
  return res.json() as Promise<T>;
};
```

- [ ] **Step 4: Create index barrel**

```typescript
// desktop-ui/src/shared/ipc/index.ts
export * from "./dispatch";
export * from "./bindings";
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd desktop-ui && bun run test dispatch`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/shared/ipc/
git commit -m "feat(desktop-ui): add typed IPC dispatch adapter"
```

---

### Task 4: Annotate all DTOs with `specta::Type`

**Files:**
- Modify: `crates/desktop-shared/src/**/*.rs` (all public DTO types)
- Modify: `crates/app-core/src/**/*.rs` (response/request types crossing IPC boundary)
- Modify: `crates/storage/src/**/*.rs` (`*Row` types exposed via handlers)
- Test: `crates/desktop-shared/tests/dto_roundtrip.rs` (new)

- [ ] **Step 1: Write the roundtrip test**

```rust
// crates/desktop-shared/tests/dto_roundtrip.rs
//! Serde <-> specta roundtrip checks — catches type drift.

macro_rules! assert_roundtrip {
    ($t:ty, $sample:expr) => {{
        let v: $t = $sample;
        let json = serde_json::to_string(&v).unwrap();
        let back: $t = serde_json::from_str(&json).unwrap();
        assert_eq!(format!("{:?}", v), format!("{:?}", back));
    }};
}

#[test]
fn task_row_roundtrip() {
    use desktop_shared::TaskRow; // replace with actual path
    assert_roundtrip!(TaskRow, TaskRow::default());
}
// ...one test per public DTO, add as you annotate them.
```

Start with one DTO (e.g., `TaskRow`). The test will fail until Step 2.

- [ ] **Step 2: Add `#[derive(specta::Type)]` to DTOs**

For each public DTO (`TaskRow`, `ProjectRow`, `NoteRow`, `AreaRow`, `OkrRow`, all `*Response`, `*Params`, `*Request` types), add `specta::Type` to the derive list:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TaskRow { /* ... */ }
```

Use `grep -rn "#\[derive.*Serialize" crates/desktop-shared/src/` to find candidates. Also `crates/app-core/src/` and `crates/storage/src/`.

For types using `chrono::DateTime<Utc>`, specta handles it via the `chrono` feature (already enabled). For custom types that wrap them, annotate with `#[specta(type = chrono::DateTime<chrono::Utc>)]` if needed.

- [ ] **Step 3: Add one roundtrip test per major DTO**

Aim for: `TaskRow`, `ProjectRow`, `NoteRow`, `AreaRow`, `OkrRow`, `EntityRow`, plus all request/response types used in top 20 commands. ~25 tests total.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p desktop-shared`
Expected: all roundtrip tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/ crates/app-core/ crates/storage/
git commit -m "feat: annotate IPC DTOs with specta::Type"
```

---

### Task 5: Annotate all Tauri commands with `#[specta::specta]`

**Files:**
- Modify: all 49 files in `crates/desktop/src/commands/*.rs`
- Modify: `crates/desktop/src/lib.rs` (expand `collect_commands![]`)

- [ ] **Step 1: Annotate commands file-by-file**

For each file in `crates/desktop/src/commands/`, add `#[specta::specta]` under every `#[tauri::command]`:
```rust
#[tauri::command]
#[specta::specta]
pub async fn task_list(state: tauri::State<'_, AppState>) -> Result<Vec<TaskRow>, KlyntbotError> {
    // ...
}
```

Commands that take `tauri::State`, `tauri::AppHandle`, or `tauri::Window` are supported by tauri-specta — no changes needed beyond the attribute.

- [ ] **Step 2: Register every command in `collect_commands![]`**

In `crates/desktop/src/lib.rs`, expand the macro call to list every command. Use this script to generate the list:
```bash
grep -rn "pub async fn\|pub fn" crates/desktop/src/commands/ | \
  grep -B1 "#\[tauri::command\]" | \
  grep "pub " | \
  awk '{print $3}' | sed 's/(.*//' | sort -u
```

Output format:
```rust
.commands(collect_commands![
    commands::tasks::task_list,
    commands::tasks::task_create,
    commands::tasks::task_update,
    // ... all ~200 commands
])
```

- [ ] **Step 3: Rebuild to regenerate bindings**

Run: `cargo build -p desktop`
Expected: success. `bindings.ts` now contains all commands as `commands.taskList`, `commands.taskCreate`, etc.

- [ ] **Step 4: Typecheck the generated bindings**

Run: `cd desktop-ui && bunx tsc --noEmit`
Expected: success (bindings not yet consumed by call sites, so no errors).

- [ ] **Step 5: Extend the dev-server coverage test**

Modify `crates/desktop/src/dev_server/mod.rs` (in the existing test module). Add an assertion that every command in `collect_commands![]` has a matching dev-server route:
```rust
#[test]
fn every_specta_command_has_dev_server_route() {
    let specta_commands = specta_command_names(); // from tauri_specta::Builder
    let dev_routes = dev_server_routes();
    for cmd in specta_commands {
        assert!(
            dev_routes.contains(&cmd),
            "command {cmd} has no dev-server route"
        );
    }
}
```

(If extracting `specta_command_names()` from the builder is awkward, generate a static `pub const ALL_COMMANDS: &[&str]` at codegen time and compare against that.)

- [ ] **Step 6: Run the extended test**

Run: `cargo nextest run -p desktop dev_server`
Expected: PASS. If any command is missing, add the route in `dev_server/mod.rs`.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/
git commit -m "feat(desktop): annotate all Tauri commands for specta codegen"
```

---

### Task 6: CI drift check for bindings.ts

**Files:**
- Modify: `.github/workflows/ci.yml` (or equivalent) — if no CI yet, create the check as a `justfile` / `cargo xtask` target

- [ ] **Step 1: Add CI step**

In the CI workflow, after the Rust build step:
```yaml
- name: Verify bindings.ts is up to date
  run: |
    cargo build -p desktop
    git diff --exit-code desktop-ui/src/shared/ipc/bindings.ts
```

If the project has no CI yet, create `xtask/src/main.rs` with a `check-bindings` subcommand that runs the same check locally.

- [ ] **Step 2: Verify locally**

Run: `cargo build -p desktop && git diff --exit-code desktop-ui/src/shared/ipc/bindings.ts`
Expected: exit code 0.

- [ ] **Step 3: Commit**

```bash
git add .github/
git commit -m "ci: verify bindings.ts stays in sync with Rust"
```

---

## Phase 2 — TanStack Query migration

### Task 7: Install TanStack Query

**Files:**
- Modify: `desktop-ui/package.json`

- [ ] **Step 1: Install packages**

Run:
```bash
cd desktop-ui && bun add @tanstack/react-query@^5 @tanstack/react-query-devtools@^5
```

- [ ] **Step 2: Verify install**

Run: `cd desktop-ui && bun run build`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lock
git commit -m "deps: add @tanstack/react-query"
```

---

### Task 8: Wire QueryClientProvider at app root

**Files:**
- Modify: `desktop-ui/src/app/App.tsx` (or wherever the root React tree lives)
- Create: `desktop-ui/src/shared/query/client.ts`

- [ ] **Step 1: Create the shared QueryClient**

```typescript
// desktop-ui/src/shared/query/client.ts
import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});
```

- [ ] **Step 2: Wrap the app**

In `App.tsx` (locate with `find desktop-ui/src -name "App.tsx"`), wrap the root:
```tsx
import { QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { queryClient } from "@shared/query/client";

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      {/* existing tree */}
      {import.meta.env.DEV && <ReactQueryDevtools buttonPosition="bottom-right" />}
    </QueryClientProvider>
  );
}
```

- [ ] **Step 3: Run the app**

Run: `cd desktop-ui && bun run dev` (in background) and `cargo tauri dev`.
Expected: app loads, devtools button appears bottom-right in dev mode.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/
git commit -m "feat(desktop-ui): mount QueryClientProvider"
```

---

### Task 9: Replace `useQuery` hook with TanStack wrapper

**Files:**
- Modify: `desktop-ui/src/shared/hooks/useQuery.ts`
- Create: `desktop-ui/src/shared/hooks/useQuery.test.ts`

- [ ] **Step 1: Write a test for the new signature**

```typescript
// desktop-ui/src/shared/hooks/useQuery.test.ts
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { useQuery } from "./useQuery";

function wrapper({ client }: { client: QueryClient }) {
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

describe("useQuery", () => {
  it("invokes dispatch with cmd name and args, returns data", async () => {
    vi.doMock("@shared/ipc", () => ({
      dispatch: vi.fn().mockResolvedValue({ version: "0.1.0" }),
    }));
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const { result } = renderHook(
      () => useQuery("get_app_version", {}),
      { wrapper: wrapper({ client }) },
    );
    await waitFor(() => expect(result.current.data).toEqual({ version: "0.1.0" }));
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBe(null);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd desktop-ui && bun run test useQuery`
Expected: FAIL (not yet refactored).

- [ ] **Step 3: Replace implementation**

```typescript
// desktop-ui/src/shared/hooks/useQuery.ts
import { useQuery as useTanstackQuery } from "@tanstack/react-query";
import { dispatch } from "@shared/ipc";
import type { ApiError } from "@shared/types";

export interface QueryResult<T> {
  data: T | undefined;
  loading: boolean;
  error: ApiError | null;
  refetch: () => void;
}

export interface UseQueryOptions {
  staleTime?: number;
  enabled?: boolean;
}

export function useQuery<T>(cmd: string, args: Record<string, unknown> = {}, opts: UseQueryOptions = {}): QueryResult<T> {
  const q = useTanstackQuery<T, ApiError>({
    queryKey: [cmd, args],
    queryFn: () => dispatch<T>(cmd, args),
    staleTime: opts.staleTime ?? 30_000,
    enabled: opts.enabled,
  });
  return {
    data: q.data,
    loading: q.isLoading,
    error: (q.error as ApiError | null) ?? null,
    refetch: () => void q.refetch(),
  };
}
```

Note: the old `invalidateOn`/`invalidateFilter` options are removed. Invalidation now happens via Phase 3's `EntitySyncProvider`.

- [ ] **Step 4: Run test**

Run: `cd desktop-ui && bun run test useQuery`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/shared/hooks/useQuery.ts desktop-ui/src/shared/hooks/useQuery.test.ts
git commit -m "refactor(desktop-ui): back useQuery with TanStack Query"
```

---

### Task 10: Replace `useMutation` hook with TanStack wrapper

**Files:**
- Modify: `desktop-ui/src/shared/hooks/useMutation.ts`
- Create: `desktop-ui/src/shared/hooks/useMutation.test.ts`

- [ ] **Step 1: Write the test**

```typescript
// desktop-ui/src/shared/hooks/useMutation.test.ts
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { useMutation } from "./useMutation";

function wrapper({ client }: { client: QueryClient }) {
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

describe("useMutation", () => {
  it("invokes dispatch and returns data", async () => {
    vi.doMock("@shared/ipc", () => ({
      dispatch: vi.fn().mockResolvedValue({ id: "t1" }),
    }));
    const client = new QueryClient();
    const { result } = renderHook(
      () => useMutation<{ id: string }, { title: string }>("task_create"),
      { wrapper: wrapper({ client }) },
    );
    let output: { id: string } | undefined;
    await act(async () => {
      output = await result.current.mutate({ title: "x" });
    });
    expect(output).toEqual({ id: "t1" });
    await waitFor(() => expect(result.current.loading).toBe(false));
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd desktop-ui && bun run test useMutation`
Expected: FAIL.

- [ ] **Step 3: Replace implementation**

```typescript
// desktop-ui/src/shared/hooks/useMutation.ts
import { useMutation as useTanstackMutation } from "@tanstack/react-query";
import { dispatch } from "@shared/ipc";
import type { ApiError } from "@shared/types";

export interface MutationResult<T, P> {
  mutate: (params: P) => Promise<T | undefined>;
  loading: boolean;
  error: ApiError | null;
}

export interface UseMutationOptions {
  /** Optional: wrap args under this key (for Tauri commands that take a struct). */
  wrapKey?: string;
}

export function useMutation<T, P = void>(cmd: string, opts: UseMutationOptions = {}): MutationResult<T, P> {
  const m = useTanstackMutation<T, ApiError, P>({
    mutationFn: async (params: P) => {
      const args = opts.wrapKey ? { [opts.wrapKey]: params } : (params as Record<string, unknown>);
      return dispatch<T>(cmd, args ?? {});
    },
  });
  return {
    mutate: async (params: P) => {
      try {
        return await m.mutateAsync(params);
      } catch {
        return undefined;
      }
    },
    loading: m.isPending,
    error: (m.error as ApiError | null) ?? null,
  };
}
```

The old `inferEntityKind()` and `entity:updated` emission are gone — Phase 3 handles this.

- [ ] **Step 4: Run test**

Run: `cd desktop-ui && bun run test useMutation`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/shared/hooks/useMutation.ts desktop-ui/src/shared/hooks/useMutation.test.ts
git commit -m "refactor(desktop-ui): back useMutation with TanStack Query"
```

---

### Task 11: Delete old custom cache and verify app still works

**Files:**
- Delete: `desktop-ui/src/shared/hooks/useIpc.ts` (replaced by `@shared/ipc`)
- Modify: `desktop-ui/src/shared/hooks/index.ts`
- Modify: all files importing from `@shared/hooks/useIpc`

- [ ] **Step 1: Find all importers**

Run: `grep -rn "from.*useIpc\|from.*@shared/hooks/useIpc\|from.*hooks/useIpc" desktop-ui/src`
Record the file list.

- [ ] **Step 2: Update imports**

For each file, change:
```typescript
import { ipc } from "@shared/hooks/useIpc";
```
to:
```typescript
import { dispatch as ipc } from "@shared/ipc";
```
(Alias keeps call sites unchanged for this task; Task 14 replaces with typed commands.)

- [ ] **Step 3: Delete the old file**

```bash
rm desktop-ui/src/shared/hooks/useIpc.ts
```

Update `desktop-ui/src/shared/hooks/index.ts` to remove the `useIpc` export.

- [ ] **Step 4: Typecheck**

Run: `cd desktop-ui && bunx tsc --noEmit`
Expected: success.

- [ ] **Step 5: Run all FE tests**

Run: `cd desktop-ui && bun run test`
Expected: PASS.

- [ ] **Step 6: Smoke-test the app**

Run: `cd desktop-ui && bun run dev` (background) and `cargo tauri dev`. Open the app; navigate through a few views (tasks, notes, projects). Verify:
- Data loads.
- Mutations complete without error.
- React Query Devtools shows cached queries.

Queries will NOT auto-refresh on mutation yet — that's Phase 3.

- [ ] **Step 7: Commit**

```bash
git add -A desktop-ui/src/
git commit -m "refactor(desktop-ui): delete custom cache, migrate to @shared/ipc"
```

---

### Task 12: Also replace `features/chat/hooks/useIpc.ts`

**Files:**
- Delete: `desktop-ui/src/features/chat/hooks/useIpc.ts`
- Modify: chat feature files that import it

- [ ] **Step 1: Find importers**

Run: `grep -rn "features/chat/hooks/useIpc" desktop-ui/src`

- [ ] **Step 2: Rewire to shared dispatch**

Replace imports with `import { dispatch as ipc } from "@shared/ipc";`.

- [ ] **Step 3: Delete the feature-local copy**

```bash
rm desktop-ui/src/features/chat/hooks/useIpc.ts
```

- [ ] **Step 4: Typecheck + test**

Run: `cd desktop-ui && bunx tsc --noEmit && bun run test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A desktop-ui/src/features/chat/
git commit -m "refactor(desktop-ui): consolidate chat IPC onto @shared/ipc"
```

---

### Task 13: Big-bang sweep — migrate call sites to typed commands

**Files:**
- Modify: every file in `desktop-ui/src` currently calling `ipc("cmd_name", ...)` or `useQuery("cmd_name", ...)` or `useMutation("cmd_name", ...)`

Strategy: do this one feature folder at a time, committing after each, so bisect stays clean.

- [ ] **Step 1: Enumerate sweep units**

Run: `ls desktop-ui/src/features/`
Each folder is one sweep unit. Also sweep `desktop-ui/src/app/` and `desktop-ui/src/shared/` (non-IPC parts).

- [ ] **Step 2: For each sweep unit, convert calls**

Pattern:
```typescript
// before
const { data } = useQuery<TaskRow[]>("task_list", {});

// after
import { commands } from "@shared/ipc";
const { data } = useQuery(commands.taskList, {});
```

This requires updating `useQuery`/`useMutation` to accept either a string command name OR a typed binding (because both shapes coexist only briefly — we standardize on typed). Since Task 9/10 already took a string, extend them now:

```typescript
// desktop-ui/src/shared/hooks/useQuery.ts — accept TypedCommand | string
type CommandLike<T> = string | { name: string; (args: unknown): Promise<T> };

export function useQuery<T>(cmd: CommandLike<T>, args = {}, opts = {}): QueryResult<T> {
  const cmdName = typeof cmd === "string" ? cmd : cmd.name;
  const queryFn = typeof cmd === "string"
    ? () => dispatch<T>(cmd, args)
    : () => (cmd as (a: unknown) => Promise<T>)(args);
  // ... rest unchanged, use cmdName in queryKey
}
```

Update tauri-specta codegen to produce callable functions with a `.name` property attached. If specta-typescript doesn't do this natively, post-process `bindings.ts` in `build.rs` to attach `commandFn.name = "command_name"` for each.

- [ ] **Step 3: Typecheck after each feature folder**

Run: `cd desktop-ui && bunx tsc --noEmit`
Expected: success after each folder.

- [ ] **Step 4: Commit per feature folder**

Example:
```bash
git add desktop-ui/src/features/database/
git commit -m "refactor(database): migrate to typed IPC bindings"
```

Repeat for: `database/`, `chat/`, `tasks/`, `notes/`, `projects/`, `cognitive/`, `focus/`, `finance/`, `learning/`, `language/`, `settings/`, `app/`.

- [ ] **Step 5: Final typecheck + tests + lint**

Run:
```bash
cd desktop-ui && bunx tsc --noEmit && bun run test && bun run lint
```
Expected: all green.

- [ ] **Step 6: Final smoke test**

Run the app. Hit every major view. Verify no runtime errors and all queries populate.

---

### Task 14: Remove the string-command overload

**Files:**
- Modify: `desktop-ui/src/shared/hooks/useQuery.ts`
- Modify: `desktop-ui/src/shared/hooks/useMutation.ts`
- Modify: tests

- [ ] **Step 1: Drop the string overload**

After Task 13's sweep, no call site passes a string. Remove the `CommandLike` union; accept only typed command functions:

```typescript
type TypedCommand<TArgs, T> = ((args: TArgs) => Promise<T>) & { name: string };

export function useQuery<TArgs, T>(cmd: TypedCommand<TArgs, T>, args: TArgs, opts = {}): QueryResult<T> {
  // ...
}
```

- [ ] **Step 2: Update tests accordingly**

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bunx tsc --noEmit`
Expected: success. Any missed call site surfaces here.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/shared/hooks/
git commit -m "refactor(desktop-ui): enforce typed command bindings only"
```

---

## Phase 3 — Push invalidation

### Task 15: Define `EntityUpdate` types in Rust

**Files:**
- Create: `crates/desktop-shared/src/events.rs`
- Modify: `crates/desktop-shared/src/lib.rs`
- Test: `crates/desktop-shared/tests/entity_update_roundtrip.rs`

- [ ] **Step 1: Write the roundtrip test**

```rust
// crates/desktop-shared/tests/entity_update_roundtrip.rs
use desktop_shared::events::{EntityKind, EntityOp, EntityUpdate};

#[test]
fn id_op_roundtrips() {
    let v = EntityUpdate::IdOp { kind: EntityKind::Task, id: "t1".into(), op: EntityOp::Update };
    let json = serde_json::to_string(&v).unwrap();
    let back: EntityUpdate = serde_json::from_str(&json).unwrap();
    match back {
        EntityUpdate::IdOp { kind, id, op } => {
            assert!(matches!(kind, EntityKind::Task));
            assert_eq!(id, "t1");
            assert!(matches!(op, EntityOp::Update));
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn full_payload_roundtrips() {
    let v = EntityUpdate::FullPayload {
        kind: EntityKind::Note,
        payload: serde_json::json!({"id": "n1", "title": "hi"}),
    };
    let json = serde_json::to_string(&v).unwrap();
    let back: EntityUpdate = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, EntityUpdate::FullPayload { .. }));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo nextest run -p desktop-shared entity_update_roundtrip`
Expected: FAIL (module missing).

- [ ] **Step 3: Implement types**

```rust
// crates/desktop-shared/src/events.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Task,
    Project,
    Note,
    Notebook,
    Area,
    Okr,
    KeyResult,
    Group,
    Squad,
    Entity,
    EntityLink,
    Annotation,
    ProjectConversation,
    ProjectMemory,
    ProjectSource,
    // extend as features register new entities
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EntityOp { Create, Update, Delete }

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum EntityUpdate {
    IdOp { kind: EntityKind, id: String, op: EntityOp },
    FullPayload { kind: EntityKind, payload: serde_json::Value },
}
```

In `lib.rs`, add `pub mod events;`.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p desktop-shared`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/
git commit -m "feat(desktop-shared): define EntityUpdate event type"
```

---

### Task 16: Replace `emit_updates` with typed event emission

**Files:**
- Modify: `crates/app-core/src/events.rs` (or wherever `emit_updates` lives today; locate via `grep -rn "fn emit_updates"`)
- Modify: `crates/desktop/src/lib.rs` — register the event in `collect_events![]`

- [ ] **Step 1: Locate current emit_updates**

Run: `grep -rn "fn emit_updates\|emit_updates(" crates/`
Identify the definition site.

- [ ] **Step 2: Write a unit test**

```rust
// crates/app-core/src/events.rs (or a tests module)
#[cfg(test)]
mod tests {
    use super::*;
    use desktop_shared::events::{EntityKind, EntityOp, EntityUpdate};

    #[test]
    fn emit_collects_id_op_updates() {
        let mut sink = TestEventSink::default();
        emit_entity_update(&mut sink, EntityUpdate::IdOp {
            kind: EntityKind::Task, id: "t1".into(), op: EntityOp::Create
        });
        assert_eq!(sink.emitted.len(), 1);
    }
}
```

Adapt signature to the real `emit_updates`. `TestEventSink` is any `EventEmitter` trait impl — if none exists, abstract the existing `&AppHandle` into a trait for testability.

- [ ] **Step 3: Run test — should fail**

Run: `cargo nextest run -p app-core events`
Expected: FAIL.

- [ ] **Step 4: Rewrite emit_updates to emit typed events**

Using tauri-specta's generated event helpers:
```rust
use tauri_specta::Event;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct EntityUpdated(pub EntityUpdate);

pub fn emit_entity_update<R: tauri::Runtime>(app: &tauri::AppHandle<R>, update: EntityUpdate) -> Result<()> {
    EntityUpdated(update).emit(app).map_err(|e| KlyntbotError::internal(e.to_string()))
}

pub fn emit_updates<R: tauri::Runtime>(app: &tauri::AppHandle<R>, updates: &[EntityUpdate]) {
    for u in updates {
        if let Err(e) = emit_entity_update(app, u.clone()) {
            tracing::warn!("failed to emit entity update: {e}");
        }
    }
}
```

In `crates/desktop/src/lib.rs`, register the event:
```rust
.events(collect_events![app_core::events::EntityUpdated])
```

- [ ] **Step 5: Update all call sites of `emit_updates`**

Search: `grep -rn "emit_updates(" crates/`
For any that currently pass old event shapes (entity kind + id as separate args), convert to `Vec<EntityUpdate>`.

Also: wherever mutation handlers in `app-core` return results, ensure they build and emit `EntityUpdate::IdOp` for the affected rows.

- [ ] **Step 6: Rebuild — codegen regenerates bindings with the event**

Run: `cargo build -p desktop`
Expected: success. `bindings.ts` now contains `events.entityUpdated`.

- [ ] **Step 7: Run all Rust tests**

Run: `cargo nextest run --workspace`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/app-core/ crates/desktop/ crates/desktop-shared/
git commit -m "feat: emit typed EntityUpdate events from mutation handlers"
```

---

### Task 17: Extend dev-server SSE with EntityUpdate

**Files:**
- Modify: `crates/desktop/src/dev_server/mod.rs` (or wherever SSE streaming lives)

- [ ] **Step 1: Write parity test**

```rust
// crates/desktop/src/dev_server/tests.rs
#[tokio::test]
async fn sse_emits_entity_update_on_mutation() {
    let app = spawn_test_dev_server().await;
    let mut stream = app.sse_client("/api/events").await;

    app.post("/api/task_create", json!({"params": {"title": "x"}})).await;

    let event = stream.next_event(Duration::from_secs(2)).await.expect("timeout");
    assert_eq!(event.name, "entityUpdated");
    let payload: EntityUpdate = serde_json::from_str(&event.data).unwrap();
    assert!(matches!(payload, EntityUpdate::IdOp { kind: EntityKind::Task, op: EntityOp::Create, .. }));
}
```

- [ ] **Step 2: Run test — should fail**

Run: `cargo nextest run -p desktop dev_server sse_emits`
Expected: FAIL.

- [ ] **Step 3: Implement SSE bridging**

The dev server already streams `PipelineEvent` via SSE. Extend the emitter channel so `emit_entity_update` ALSO publishes to an `mpsc::Sender<EntityUpdate>` that the SSE handler drains and serializes as a named SSE event:
```text
event: entityUpdated
data: {"variant":"id_op","kind":"task","id":"...","op":"create"}

```

If the Tauri/dev-server paths have separate emitter traits, introduce a single `EventSink` trait with two impls (Tauri, SSE mpsc) and have `emit_entity_update` accept `&dyn EventSink`. Store whichever is live in `AppCore`.

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p desktop dev_server`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/
git commit -m "feat(dev-server): stream EntityUpdate over SSE for browser dev mode"
```

---

### Task 18: Create FE EntitySyncProvider and invalidation registry

**Files:**
- Create: `desktop-ui/src/shared/sync/EntitySyncProvider.tsx`
- Create: `desktop-ui/src/shared/sync/invalidationRegistry.ts`
- Create: `desktop-ui/src/shared/sync/fullPayloadDispatcher.ts`
- Create: `desktop-ui/src/shared/sync/index.ts`
- Create: `desktop-ui/src/shared/sync/invalidationRegistry.test.ts`
- Create: `desktop-ui/src/shared/sync/EntitySyncProvider.test.tsx`

- [ ] **Step 1: Write registry exhaustiveness test**

```typescript
// desktop-ui/src/shared/sync/invalidationRegistry.test.ts
import { describe, it, expect } from "vitest";
import { invalidationRegistry } from "./invalidationRegistry";
import type { EntityKind } from "@shared/ipc";

describe("invalidationRegistry", () => {
  it("covers every EntityKind variant", () => {
    const kinds: EntityKind[] = [
      "task", "project", "note", "notebook", "area", "okr", "key_result",
      "group", "squad", "entity", "entity_link", "annotation",
      "project_conversation", "project_memory", "project_source",
    ];
    for (const k of kinds) {
      expect(invalidationRegistry[k], `missing registry entry for ${k}`).toBeDefined();
    }
  });
});
```

- [ ] **Step 2: Run — should fail**

Run: `cd desktop-ui && bun run test invalidationRegistry`
Expected: FAIL (module missing).

- [ ] **Step 3: Implement registry**

```typescript
// desktop-ui/src/shared/sync/invalidationRegistry.ts
import type { EntityKind } from "@shared/ipc";
import type { QueryKey } from "@tanstack/react-query";

export interface InvalidationRule {
  /** Query key prefixes to invalidate on any op. */
  listKeys: QueryKey[];
  /** Query key for a detail query keyed by id. */
  detailKey?: (id: string) => QueryKey;
}

export const invalidationRegistry: Record<EntityKind, InvalidationRule> = {
  task: {
    listKeys: [["task_list"], ["task_today"], ["task_search"]],
    detailKey: (id) => ["task_get", { id }],
  },
  project: {
    listKeys: [["project_list"]],
    detailKey: (id) => ["project_get", { id }],
  },
  note: {
    listKeys: [["note_list"], ["inbox_list"]],
    detailKey: (id) => ["note_get", { id }],
  },
  notebook: { listKeys: [["notebook_list"]] },
  area: { listKeys: [["area_list"]], detailKey: (id) => ["area_get", { id }] },
  okr: { listKeys: [["okr_list"]], detailKey: (id) => ["okr_get", { id }] },
  key_result: { listKeys: [["key_result_list"]] },
  group: { listKeys: [["group_list"]] },
  squad: { listKeys: [["squad_list"]] },
  entity: { listKeys: [["entity_list"]], detailKey: (id) => ["entity_get", { id }] },
  entity_link: { listKeys: [["entity_links"]] },
  annotation: { listKeys: [["annotation_list"]] },
  project_conversation: { listKeys: [["project_conversation_list"]] },
  project_memory: { listKeys: [["project_memory_list"]] },
  project_source: { listKeys: [["project_source_list"]] },
};
```

Adjust list keys after grepping the codebase for actual `useQuery` keys used today.

- [ ] **Step 4: Implement full-payload dispatcher**

```typescript
// desktop-ui/src/shared/sync/fullPayloadDispatcher.ts
import type { QueryClient } from "@tanstack/react-query";
import type { EntityKind } from "@shared/ipc";
import { invalidationRegistry } from "./invalidationRegistry";

export function applyFullPayload(qc: QueryClient, kind: EntityKind, payload: unknown) {
  const rule = invalidationRegistry[kind];
  if (!rule) {
    console.warn(`[sync] no registry entry for kind=${kind}`);
    return;
  }
  const id = (payload as { id?: string })?.id;
  if (id && rule.detailKey) {
    qc.setQueryData(rule.detailKey(id), payload);
  }
  // List caches: invalidate — optimistic in-place patching is out of scope here.
  for (const key of rule.listKeys) {
    qc.invalidateQueries({ queryKey: key });
  }
}
```

- [ ] **Step 5: Implement EntitySyncProvider**

```tsx
// desktop-ui/src/shared/sync/EntitySyncProvider.tsx
import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { events, type EntityUpdate } from "@shared/ipc";
import { invalidationRegistry } from "./invalidationRegistry";
import { applyFullPayload } from "./fullPayloadDispatcher";

export function EntitySyncProvider({ children }: { children: React.ReactNode }) {
  const qc = useQueryClient();

  useEffect(() => {
    const unlistenPromise = events.entityUpdated.listen((ev) => {
      const update = ev.payload as EntityUpdate;
      if (update.variant === "id_op") {
        const rule = invalidationRegistry[update.kind];
        if (!rule) { console.warn(`[sync] no registry entry for ${update.kind}`); return; }
        for (const key of rule.listKeys) qc.invalidateQueries({ queryKey: key });
        if (rule.detailKey) qc.invalidateQueries({ queryKey: rule.detailKey(update.id) });
      } else if (update.variant === "full_payload") {
        applyFullPayload(qc, update.kind, update.payload);
      }
    });
    return () => { unlistenPromise.then((u) => u()); };
  }, [qc]);

  return <>{children}</>;
}
```

- [ ] **Step 6: Barrel**

```typescript
// desktop-ui/src/shared/sync/index.ts
export { EntitySyncProvider } from "./EntitySyncProvider";
export { invalidationRegistry } from "./invalidationRegistry";
```

- [ ] **Step 7: Run tests**

Run: `cd desktop-ui && bun run test sync`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/shared/sync/
git commit -m "feat(desktop-ui): EntitySyncProvider + invalidation registry"
```

---

### Task 19: Mount EntitySyncProvider at app root

**Files:**
- Modify: `desktop-ui/src/app/App.tsx`

- [ ] **Step 1: Wrap under QueryClientProvider**

```tsx
import { EntitySyncProvider } from "@shared/sync";

<QueryClientProvider client={queryClient}>
  <EntitySyncProvider>
    {/* existing tree */}
  </EntitySyncProvider>
  {import.meta.env.DEV && <ReactQueryDevtools buttonPosition="bottom-right" />}
</QueryClientProvider>
```

- [ ] **Step 2: Smoke test**

Run the app. Edit a task in one view, switch to another view that shows the same task — it should update without manual refetch. Watch the TanStack Query Devtools: when the Rust event arrives, the relevant queries should flip to `inactive → fetching`.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/app/App.tsx
git commit -m "feat(desktop-ui): mount EntitySyncProvider at app root"
```

---

### Task 20: Remove all manual `invalidateQueries` call sites

**Files:**
- Modify: every file currently calling `queryClient.invalidateQueries` outside `shared/sync/`

- [ ] **Step 1: Find all call sites**

Run: `grep -rn "invalidateQueries\|refetch()" desktop-ui/src --include="*.ts" --include="*.tsx" | grep -v "shared/sync"`

- [ ] **Step 2: For each, delete the manual invalidation**

The server event now handles it. Confirm by reading the mutation — if `onSuccess` only did invalidation, delete the `onSuccess`. Keep any UI-side logic (toast, navigation).

- [ ] **Step 3: Add a CI grep check**

Create `desktop-ui/scripts/check-no-manual-invalidation.sh`:
```bash
#!/usr/bin/env bash
set -euo pipefail
matches=$(grep -rn "invalidateQueries" desktop-ui/src --include="*.ts" --include="*.tsx" | grep -v "shared/sync" || true)
if [ -n "$matches" ]; then
  echo "Manual invalidateQueries outside shared/sync is forbidden:"
  echo "$matches"
  exit 1
fi
```

Wire into CI (or add a `bun run lint:no-manual-invalidation` script and include in `bun run lint`).

- [ ] **Step 4: Run checks**

Run: `bash desktop-ui/scripts/check-no-manual-invalidation.sh && cd desktop-ui && bun run test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(desktop-ui): centralize invalidation via EntitySyncProvider"
```

---

### Task 21: End-to-end smoke test

**Files:**
- Create: `desktop-ui/src/__e2e__/push-invalidation.test.tsx` (or wherever the project's integration tests live)

- [ ] **Step 1: Write the test**

```tsx
// Uses @testing-library/react + a mock Tauri event bridge or the real dev server.
// Minimal version:
import { describe, it, expect } from "vitest";
import { render, screen, act, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { EntitySyncProvider } from "@shared/sync";

describe("push invalidation e2e", () => {
  it("updates query cache when entityUpdated event fires", async () => {
    // mock events.entityUpdated.listen + dispatch
    // render a component that useQuery("task_list")
    // fire an event
    // assert refetch happened
  });
});
```

Concretize against the project's actual test harness.

- [ ] **Step 2: Run it**

Run: `cd desktop-ui && bun run test push-invalidation`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/__e2e__/
git commit -m "test: e2e push invalidation smoke test"
```

---

### Task 22: Full verification pass

- [ ] **Step 1: Rust**

Run: `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all --check`
Expected: all green.

- [ ] **Step 2: Frontend**

Run: `cd desktop-ui && bunx tsc --noEmit && bun run test && bun run lint`
Expected: all green.

- [ ] **Step 3: Bindings freshness**

Run: `cargo build -p desktop && git diff --exit-code desktop-ui/src/shared/ipc/bindings.ts`
Expected: exit 0.

- [ ] **Step 4: Manual smoke**

`cargo tauri dev` — open the app, verify:
- Every major feature (tasks, projects, notes, okr, finance, cognitive) loads data.
- Creating/editing an entity in one view updates another view showing the same data without manual refresh.
- React Query Devtools shows queries.
- No errors in browser console or `tauri dev` terminal.

- [ ] **Step 5: Final commit & summary**

If any fixup needed:
```bash
git add -A
git commit -m "chore: typed IPC + TanStack Query + push invalidation complete"
```

---

## Self-review notes

- **Spec coverage:** Phase 1 (typed bindings) → Tasks 1-6. Phase 2 (TanStack Query) → Tasks 7-14. Phase 3 (push invalidation) → Tasks 15-21. Verification → Task 22. Every spec section maps to a task.
- **Big-bang sweep:** Task 13 is the large one. Split per feature folder to keep diff reviewable and bisect clean.
- **Type consistency:** `EntityUpdate`, `EntityKind`, `EntityOp` defined in Task 15 are referenced in Tasks 16-21 with the same names.
- **Risk concentration:** Task 5 (annotate all commands) and Task 13 (migrate all call sites) are the two biggest risk areas. Both are mechanical; failure mode is typecheck errors rather than runtime breakage.
- **Rollback:** Every task commits independently. Revert individual commits if something breaks. No migrations, no users — safe.
