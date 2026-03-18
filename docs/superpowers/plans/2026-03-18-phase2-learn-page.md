# Phase 2: Learning Hub Page (`/learn`) — Dashboard + Immersive Review

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `/learn` top-level route with a dashboard home (stats, decks, quick actions) and an immersive full-screen review mode. Wire the FSRS-5 backend from Phase 1 into a working frontend — the user should be able to review due cards with proper FSRS-5 scheduling, see deck stats, and create cards manually.

**Architecture:** New `desktop-ui/src/features/learn/` feature directory with its own page, components, and hooks. New backend commands for missing CRUD operations (create single card, update card, list all cards in deck, get single card, delete card). Route registered as a sidebar peer to Notes. Dashboard shows deck stats + due counts; clicking "Start Review" enters immersive keyboard-driven review mode.

**Tech Stack:** React, TypeScript, TipTap (not needed yet), Tailwind v4 + CSS tokens, lucide-react icons, recharts (for mini charts), existing `ipc`/`useQuery`/`useMutation` hooks. Rust: sqlx, Tauri commands.

**Spec:** `docs/superpowers/specs/2026-03-18-learning-system-design.md` — Sections: "Learning Hub UI", "Data Model"

**Phase 1 prerequisite:** FSRS-5 engine, card types, review_log — already implemented.

---

## File Map

### Backend (new commands + handlers)

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `crates/cognitive/src/repos/flashcard.rs` | Add `get_by_id`, `create_single`, `update_card`, `list_all_in_deck`, `delete_card`, `suspend_card`, `total_due_count` |
| Modify | `crates/app-core/src/handlers/notes/flashcard.rs` | Add handler methods for new operations |
| Modify | `crates/desktop-shared/src/commands/notes.rs` | Add `FlashcardCreateParams`, `FlashcardUpdateParams`, `FlashcardListParams` |
| Modify | `crates/desktop/src/commands/notes.rs` | Add Tauri commands + DEV_COMMANDS + dispatch_dev |
| Modify | `crates/desktop/src/main.rs` | Register new commands in `generate_handler!` |

### Frontend (new feature)

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `desktop-ui/src/features/learn/index.ts` | Barrel export for `LearnPage` |
| Create | `desktop-ui/src/features/learn/pages/LearnPage.tsx` | Top-level page: dashboard home + immersive review mode switcher |
| Create | `desktop-ui/src/features/learn/components/DashboardHome.tsx` | Stats bar, deck list, quick actions |
| Create | `desktop-ui/src/features/learn/components/StatsBar.tsx` | Streak, due count, retention %, weekly count |
| Create | `desktop-ui/src/features/learn/components/DeckList.tsx` | Per-deck stats with review buttons |
| Create | `desktop-ui/src/features/learn/components/ImmersiveReview.tsx` | Full-screen card review with keyboard shortcuts |
| Create | `desktop-ui/src/features/learn/components/CardRenderer.tsx` | Renders all 5 card types (basic, cloze, vocab, typed, image_occlusion) |
| Create | `desktop-ui/src/features/learn/components/RatingButtons.tsx` | Again/Hard/Good/Easy with computed intervals |
| Create | `desktop-ui/src/features/learn/components/QuickAdd.tsx` | ⌘N modal for manual card creation |
| Create | `desktop-ui/src/features/learn/components/PostSession.tsx` | Session summary after review |
| Create | `desktop-ui/src/features/learn/hooks/useLearnDashboard.ts` | Dashboard data fetching (decks, stats, due counts) |
| Create | `desktop-ui/src/features/learn/hooks/useReviewSession.ts` | Review session state + recall timing |
| Modify | `desktop-ui/src/shared/types/common.ts:L56-L66` | Add `"Learn"` to `SidebarItem` union |
| Modify | `desktop-ui/src/app/layouts/Sidebar.tsx:L23-L32` | Add Learn entry to sidebar items |
| Modify | `desktop-ui/src/app/layouts/AppShell.tsx:L61-L71` | Add `/learn` path matching |
| Modify | `desktop-ui/src/app/router.tsx` | Add lazy import + route for `/learn` |
| Modify | `desktop-ui/src/features/notes/hooks/useFlashcards.ts` | Update `Flashcard` interface for new fields (front/back, vocab_data, etc.) |

---

### Task 1: Add Missing Backend CRUD Commands

**Files:**
- Modify: `crates/cognitive/src/repos/flashcard.rs`
- Modify: `crates/app-core/src/handlers/notes/flashcard.rs`
- Modify: `crates/desktop-shared/src/commands/notes.rs`
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `crates/desktop/src/main.rs`

- [ ] **Step 1: Add repo methods to FlashcardRepo**

Add these methods to `FlashcardRepo` in `crates/cognitive/src/repos/flashcard.rs`:

```rust
    /// Get a single card by ID.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<FlashcardRow>, sqlx::Error> {
        sqlx::query_as::<_, FlashcardRow>("SELECT * FROM flashcards WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Create a single flashcard (for manual creation). Immediately due.
    pub async fn create_single(&self, card: NewFlashcard) -> Result<FlashcardRow, sqlx::Error> {
        let mut rows = self.create_batch(vec![card]).await?;
        rows.pop()
            .ok_or_else(|| sqlx::Error::RowNotFound)
    }

    /// Update a card's front, back, deck, tags, and type-specific data.
    pub async fn update_card(
        &self,
        id: &str,
        front: &str,
        back: &str,
        deck: &str,
        tags: &[String],
        cloze_data: Option<&serde_json::Value>,
        vocab_data: Option<&serde_json::Value>,
    ) -> Result<FlashcardRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let tags_str = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
        let cloze_str = cloze_data.map(|v| v.to_string());
        let vocab_str = vocab_data.map(|v| v.to_string());

        sqlx::query(
            r#"
            UPDATE flashcards
            SET front = ?1, back = ?2, deck = ?3, tags = ?4,
                cloze_data = ?5, vocab_data = ?6, updated_at = ?7
            WHERE id = ?8
            "#,
        )
        .bind(front)
        .bind(back)
        .bind(deck)
        .bind(&tags_str)
        .bind(&cloze_str)
        .bind(&vocab_str)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_by_id(id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// List all cards in a deck (not just due).
    pub async fn list_all_in_deck(
        &self,
        deck: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        sqlx::query_as::<_, FlashcardRow>(
            r#"
            SELECT * FROM flashcards
            WHERE deck = ?1
            ORDER BY created_at DESC
            LIMIT ?2 OFFSET ?3
            "#,
        )
        .bind(deck)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
    }

    /// Delete a single card by ID. Returns true if deleted.
    pub async fn delete_card(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM flashcards WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Toggle suspended state for a card.
    pub async fn suspend_card(&self, id: &str, suspended: bool) -> Result<FlashcardRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE flashcards SET suspended = ?1, updated_at = ?2 WHERE id = ?3",
        )
        .bind(suspended as i64)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_by_id(id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Fetch all due cards across ALL decks (for cross-deck review).
    pub async fn get_all_due_cards(&self, limit: i64) -> Result<Vec<FlashcardRow>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query_as::<_, FlashcardRow>(
            r#"
            SELECT * FROM flashcards
            WHERE suspended = 0
              AND (due_at IS NULL OR due_at <= ?1)
            ORDER BY due_at ASC
            LIMIT ?2
            "#,
        )
        .bind(&now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Get total due count across all decks.
    pub async fn total_due_count(&self) -> Result<i64, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM flashcards
            WHERE suspended = 0 AND (due_at IS NULL OR due_at <= ?1)
            "#,
        )
        .bind(&now)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }
```

- [ ] **Step 2: Run cognitive tests to verify new methods**

Run: `cargo nextest run -p cognitive -E 'test(flashcard)'`

- [ ] **Step 3: Add shared types in desktop-shared**

Add to `crates/desktop-shared/src/commands/notes.rs`:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardCreateParams {
    pub deck: String,
    pub front: String,
    pub back: String,
    pub card_type: String,
    pub tags: Option<Vec<String>>,
    pub source_note_id: Option<String>,
    pub cloze_data: Option<serde_json::Value>,
    pub vocab_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardUpdateParams {
    pub id: String,
    pub front: String,
    pub back: String,
    pub deck: String,
    pub tags: Option<Vec<String>>,
    pub cloze_data: Option<serde_json::Value>,
    pub vocab_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardListParams {
    pub deck: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
```

- [ ] **Step 4: Add app-core handler methods**

Add to `impl AppCore` in `crates/app-core/src/handlers/notes/flashcard.rs`:

```rust
    pub async fn flashcard_get(&self, id: &str) -> Result<FlashcardResponse, ApiError> {
        let repo = self.flashcard_repo.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let card = repo.get_by_id(id).await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Card not found"))?;
        Ok(flashcard_to_response(card))
    }

    pub async fn flashcard_create(&self, params: FlashcardCreateParams) -> Result<FlashcardResponse, ApiError> {
        let repo = self.flashcard_repo.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let card = cognitive::NewFlashcard {
            source_note_id: params.source_note_id,
            source_context: None,
            deck: params.deck,
            front: params.front,
            back: params.back,
            card_type: cognitive::CardType::parse(&params.card_type),
            cloze_data: params.cloze_data,
            vocab_data: params.vocab_data,
            image_data: None,
            tags: params.tags.unwrap_or_default(),
            stability: 1.0,
            difficulty: 5.0,
        };
        let row = repo.create_single(card).await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(flashcard_to_response(row))
    }

    pub async fn flashcard_update(&self, params: FlashcardUpdateParams) -> Result<FlashcardResponse, ApiError> {
        let repo = self.flashcard_repo.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let row = repo.update_card(
            &params.id, &params.front, &params.back, &params.deck,
            &params.tags.unwrap_or_default(),
            params.cloze_data.as_ref(), params.vocab_data.as_ref(),
        ).await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(flashcard_to_response(row))
    }

    pub async fn flashcard_list_cards(&self, params: FlashcardListParams) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self.flashcard_repo.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let cards = repo.list_all_in_deck(&params.deck, params.limit.unwrap_or(100), params.offset.unwrap_or(0))
            .await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(cards.into_iter().map(flashcard_to_response).collect())
    }

    pub async fn flashcard_delete(&self, id: &str) -> Result<bool, ApiError> {
        let repo = self.flashcard_repo.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        repo.delete_card(id).await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
    }

    pub async fn flashcard_get_all_due(&self, limit: i64) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self.flashcard_repo.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let cards = repo.get_all_due_cards(limit).await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(cards.into_iter().map(flashcard_to_response).collect())
    }

    pub async fn flashcard_total_due(&self) -> Result<i64, ApiError> {
        let repo = self.flashcard_repo.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        repo.total_due_count().await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
    }
```

- [ ] **Step 5: Add Tauri commands**

Add to `crates/desktop/src/commands/notes.rs` (before the `DEV_COMMANDS` block):

```rust
#[tauri::command]
pub async fn flashcard_get(state: tauri::State<'_, AppCore>, id: String) -> Result<FlashcardResponse, ApiError> {
    state.flashcard_get(&id).await
}

#[tauri::command]
pub async fn flashcard_create(state: tauri::State<'_, AppCore>, params: FlashcardCreateParams) -> Result<FlashcardResponse, ApiError> {
    state.flashcard_create(params).await
}

#[tauri::command]
pub async fn flashcard_update(state: tauri::State<'_, AppCore>, params: FlashcardUpdateParams) -> Result<FlashcardResponse, ApiError> {
    state.flashcard_update(params).await
}

#[tauri::command]
pub async fn flashcard_list_cards(state: tauri::State<'_, AppCore>, params: FlashcardListParams) -> Result<Vec<FlashcardResponse>, ApiError> {
    state.flashcard_list_cards(params).await
}

#[tauri::command]
pub async fn flashcard_delete(state: tauri::State<'_, AppCore>, id: String) -> Result<bool, ApiError> {
    state.flashcard_delete(&id).await
}

#[tauri::command]
pub async fn flashcard_get_all_due(state: tauri::State<'_, AppCore>, limit: i64) -> Result<Vec<FlashcardResponse>, ApiError> {
    state.flashcard_get_all_due(limit).await
}

#[tauri::command]
pub async fn flashcard_total_due(state: tauri::State<'_, AppCore>) -> Result<i64, ApiError> {
    state.flashcard_total_due().await
}
```

- [ ] **Step 6: Add to DEV_COMMANDS and dispatch_dev**

Add these 7 command names to `DEV_COMMANDS` array:
```rust
    "flashcard_get",
    "flashcard_create",
    "flashcard_update",
    "flashcard_list_cards",
    "flashcard_delete",
    "flashcard_get_all_due",
    "flashcard_total_due",
```

Add corresponding `dispatch_dev` match arms.

- [ ] **Step 7: Register in main.rs**

Add the 7 new commands to the `generate_handler![]` macro in `crates/desktop/src/main.rs`.

- [ ] **Step 8: Add tests for new repo methods**

Add tests to the `#[cfg(test)] mod tests` block in `flashcard.rs`:

```rust
    #[tokio::test]
    async fn test_get_by_id() {
        let (_pool, repo) = setup().await;
        let created = repo.create_batch(vec![sample_card("test", None)]).await.unwrap();
        let found = repo.get_by_id(&created[0].id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, created[0].id);

        let missing = repo.get_by_id("nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_create_single() {
        let (_pool, repo) = setup().await;
        let card = repo.create_single(sample_card("single", None)).await.unwrap();
        assert_eq!(card.front, "What is 2 + 2?");
        assert_eq!(card.state, "new");
    }

    #[tokio::test]
    async fn test_update_card() {
        let (_pool, repo) = setup().await;
        let created = repo.create_single(sample_card("edit-test", None)).await.unwrap();

        let updated = repo.update_card(
            &created.id, "Updated front", "Updated back", "new-deck",
            &["tag1".to_string()], None, None,
        ).await.unwrap();

        assert_eq!(updated.front, "Updated front");
        assert_eq!(updated.back, "Updated back");
        assert_eq!(updated.deck, "new-deck");
        let tags: Vec<String> = serde_json::from_str(&updated.tags).unwrap();
        assert_eq!(tags, vec!["tag1"]);
    }

    #[tokio::test]
    async fn test_list_all_in_deck() {
        let (_pool, repo) = setup().await;
        repo.create_batch(vec![
            sample_card("list-all", None),
            sample_card("list-all", None),
            sample_card("other", None),
        ]).await.unwrap();

        let all = repo.list_all_in_deck("list-all", 100, 0).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_card() {
        let (_pool, repo) = setup().await;
        let created = repo.create_single(sample_card("del", None)).await.unwrap();
        assert!(repo.delete_card(&created.id).await.unwrap());
        assert!(repo.get_by_id(&created.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_suspend_card() {
        let (_pool, repo) = setup().await;
        let created = repo.create_single(sample_card("suspend", None)).await.unwrap();
        assert_eq!(created.suspended, 0);

        let suspended = repo.suspend_card(&created.id, true).await.unwrap();
        assert_eq!(suspended.suspended, 1);

        // Suspended cards should not appear in due cards
        let due = repo.get_due_cards("suspend", 10).await.unwrap();
        assert!(due.is_empty());
    }

    #[tokio::test]
    async fn test_get_all_due_cards() {
        let (_pool, repo) = setup().await;
        repo.create_batch(vec![
            sample_card("deck-a", None),
            sample_card("deck-b", None),
        ]).await.unwrap();

        let all_due = repo.get_all_due_cards(100).await.unwrap();
        assert!(all_due.len() >= 2, "Should return due cards across all decks");
    }

    #[tokio::test]
    async fn test_total_due_count() {
        let (_pool, repo) = setup().await;
        repo.create_batch(vec![
            sample_card("count-a", None),
            sample_card("count-b", None),
        ]).await.unwrap();

        let count = repo.total_due_count().await.unwrap();
        assert!(count >= 2);
    }
```

- [ ] **Step 9: Verify all tests pass + workspace compiles**

Run: `cargo nextest run -p cognitive -E 'test(flashcard)'`
Run: `cargo check --workspace`
Expected: All pass, workspace compiles.

---

### Task 2: Register `/learn` Route + Sidebar Entry

**Files:**
- Modify: `desktop-ui/src/shared/types/common.ts`
- Modify: `desktop-ui/src/app/layouts/Sidebar.tsx`
- Modify: `desktop-ui/src/app/layouts/AppShell.tsx`
- Modify: `desktop-ui/src/app/router.tsx`
- Create: `desktop-ui/src/features/learn/index.ts`
- Create: `desktop-ui/src/features/learn/pages/LearnPage.tsx`

- [ ] **Step 1: Add "Learn" to SidebarItem union**

In `desktop-ui/src/shared/types/common.ts`, add `"Learn"` to the `SidebarItem` type:

```typescript
export type SidebarItem =
  | "Dashboard"
  | "Chat"
  | "Tasks"
  | "OKR"
  | "Calendar"
  | "Notes"
  | "Learn"
  | "Finance"
  | "Automations"
  | "System"
  | "Settings";
```

- [ ] **Step 2: Add Learn entry to Sidebar items**

In `desktop-ui/src/app/layouts/Sidebar.tsx`, import `GraduationCap` icon and add the Learn entry after Notes:

```typescript
import {
  CheckSquare,
  Cpu,
  FileText,
  GraduationCap,
  LayoutDashboard,
  MessageCircle,
  MessageSquare,
  Settings,
  Timer,
  Wallet,
} from "lucide-react";
```

In the `items` array, add after the Notes entry:

```typescript
  { key: "Learn", icon: GraduationCap, path: "/learn" },
```

- [ ] **Step 3: Add path matching in AppShell**

In `desktop-ui/src/app/layouts/AppShell.tsx`, add to `activeSidebarItem` useMemo (after the `/notes` line):

```typescript
    if (path.startsWith("/learn")) return "Learn";
```

- [ ] **Step 4: Create placeholder LearnPage**

Create `desktop-ui/src/features/learn/pages/LearnPage.tsx`:

```tsx
export default function LearnPage() {
  return (
    <div className="flex-1 flex items-center justify-center">
      <div className="text-center">
        <div className="text-4xl mb-2">📚</div>
        <h1 className="text-xl font-semibold text-foreground">Learning Hub</h1>
        <p className="text-muted mt-1">Coming soon</p>
      </div>
    </div>
  );
}
```

- [ ] **Step 5: Create barrel export**

Create `desktop-ui/src/features/learn/index.ts`:

```typescript
export { default as LearnPage } from "./pages/LearnPage";
```

- [ ] **Step 6: Add route in router.tsx**

In `desktop-ui/src/app/router.tsx`, add the lazy import after the Notes import:

```typescript
// ── Learn Feature ────────────────────────────────────────────────
const LearnPage = lazy(() =>
  import("../features/learn").then((m) => ({ default: m.LearnPage })),
);
```

Add the route in the children array (after the `/notes` route):

```typescript
      { path: "/learn", element: <LearnPage /> },
```

- [ ] **Step 7: Verify in browser**

Run: `cd desktop-ui && bun run dev` (if not running)
Open `http://localhost:1420/#/learn`
Expected: See the placeholder "Learning Hub — Coming soon" page. Sidebar should show the GraduationCap icon active.

---

### Task 3: Update useFlashcards Hook for New Schema

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useFlashcards.ts`

- [ ] **Step 1: Update the Flashcard interface**

Replace the `Flashcard` interface to match the new `FlashcardResponse` from Phase 1:

```typescript
export interface Flashcard {
  id: string;
  deck: string;
  front: string;
  back: string;
  cardType: string;
  clozeData: Record<string, unknown> | null;
  vocabData: {
    word?: string;
    reading?: string;
    meaning?: string;
    exampleSentence?: string;
    audioUrl?: string;
    partOfSpeech?: string;
  } | null;
  imageData: Record<string, unknown> | null;
  tags: string[];
  sourceNoteId: string | null;
  sourceContext: string | null;
  stability: number;
  difficulty: number;
  dueAt: string | null;
  state: string;
  reviewCount: number;
  recallSpeedMs: number | null;
  createdAt: string;
}
```

- [ ] **Step 2: Update the review call to pass recallSpeedMs**

Update the `review` callback to accept and pass `recallSpeedMs`:

```typescript
  const review = useCallback(
    async (quality: ReviewQuality, recallSpeedMs?: number) => {
      const card = cards[currentIndex];
      if (!card) return;
      await ipc("flashcard_record_review", {
        cardId: card.id,
        quality,
        recallSpeedMs: recallSpeedMs ?? null,
      });
      setRevealed(false);
      setCurrentIndex((i) => i + 1);
    },
    [cards, currentIndex],
  );
```

- [ ] **Step 3: Update existing FlashcardReview component for new field names**

The existing `FlashcardReview.tsx` (`desktop-ui/src/features/notes/components/insight/FlashcardReview.tsx`) references old field names. Make these specific changes:
- Rename all `current.question` → `current.front` (e.g., line ~107)
- Rename all `current.answer` → `current.back` (e.g., line ~123)
- Remove the `current.choices` rendering block (lines ~108-116) — the `choices` field no longer exists in the schema. Multiple-choice cards now use `cloze_data`/`vocab_data` instead, but rendering those is handled by the new `CardRenderer` in Task 5.
- Update any TypeScript type annotations that reference the old `Flashcard` shape.

- [ ] **Step 4: Verify in browser**

Navigate to Notes, open a note with flashcards, open Insight Review, click "Review". Verify the existing review flow still works with the updated schema.

---

### Task 4: Dashboard Home Component

**Files:**
- Create: `desktop-ui/src/features/learn/hooks/useLearnDashboard.ts`
- Create: `desktop-ui/src/features/learn/components/StatsBar.tsx`
- Create: `desktop-ui/src/features/learn/components/DeckList.tsx`
- Create: `desktop-ui/src/features/learn/components/DashboardHome.tsx`
- Modify: `desktop-ui/src/features/learn/pages/LearnPage.tsx`

- [ ] **Step 1: Create useLearnDashboard hook**

```typescript
import { useQuery } from "@shared/hooks/useQuery";
import type { DeckSummary } from "../../notes/hooks/useFlashcards";

interface DashboardData {
  decks: DeckSummary[];
  totalDue: number;
  loading: boolean;
  refetch: () => void;
}

export function useLearnDashboard(): DashboardData {
  const { data: decks, loading: decksLoading, refetch } = useQuery<DeckSummary[]>(
    "flashcard_list_decks",
    {},
    [],
  );

  const { data: totalDue, loading: dueLoading } = useQuery<number>(
    "flashcard_total_due",
    {},
    0,
  );

  return {
    decks: decks ?? [],
    totalDue: totalDue ?? 0,
    loading: decksLoading || dueLoading,
    refetch,
  };
}
```

- [ ] **Step 2: Create StatsBar component**

A horizontal bar showing streak, due count, retention %, weekly count. Use the glass-card pattern from the design system. Initially, streak and retention are placeholders (0 / --%) since we don't have study_sessions table yet (Phase 5). Due count is live.

- [ ] **Step 3: Create DeckList component**

Shows each deck with name, card count, due count, and a "Review" button. Clicking "Review" triggers `onStartReview(deckName)`.

- [ ] **Step 4: Create DashboardHome component**

Composes StatsBar + DeckList + empty states. Has a large "Start Review" button that reviews ALL due cards (across decks), and a "Quick Add" button (⌘N).

- [ ] **Step 5: Update LearnPage to render DashboardHome**

Replace the placeholder with the real dashboard. The page manages view mode state: `"dashboard" | "review"`. Dashboard is the default. When user clicks "Start Review", switches to review mode (Task 5).

- [ ] **Step 6: Verify in browser**

Navigate to `/learn`. Should see the dashboard with stats bar, deck list (may be empty if no cards exist). "Start Review" button should be visible.

---

### Task 5: Immersive Review Mode

**Files:**
- Create: `desktop-ui/src/features/learn/hooks/useReviewSession.ts`
- Create: `desktop-ui/src/features/learn/components/CardRenderer.tsx`
- Create: `desktop-ui/src/features/learn/components/RatingButtons.tsx`
- Create: `desktop-ui/src/features/learn/components/ImmersiveReview.tsx`
- Create: `desktop-ui/src/features/learn/components/PostSession.tsx`
- Modify: `desktop-ui/src/features/learn/pages/LearnPage.tsx`

- [ ] **Step 1: Create useReviewSession hook**

Manages review session state with recall timing:

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useRef, useState } from "react";
import type { Flashcard } from "../../notes/hooks/useFlashcards";

type ReviewQuality = "again" | "hard" | "good" | "easy";

interface ReviewSession {
  cards: Flashcard[];
  currentIndex: number;
  current: Flashcard | null;
  revealed: boolean;
  done: boolean;
  remaining: number;
  totalReviewed: number;
  correctCount: number;
  startTime: number;
  startReview: (deck?: string) => Promise<void>;
  reveal: () => void;
  rate: (quality: ReviewQuality) => Promise<void>;
  exit: () => void;
}

export function useReviewSession(onExit: () => void): ReviewSession {
  const [cards, setCards] = useState<Flashcard[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [revealed, setRevealed] = useState(false);
  const [totalReviewed, setTotalReviewed] = useState(0);
  const [correctCount, setCorrectCount] = useState(0);
  const [startTime] = useState(Date.now());
  const cardShownAt = useRef<number>(0);

  const startReview = useCallback(async (deck?: string) => {
    const due = deck
      ? await ipc<Flashcard[]>("flashcard_get_due", { deck, limit: 50 })
      : await ipc<Flashcard[]>("flashcard_get_all_due", { limit: 50 });
    setCards(due);
    setCurrentIndex(0);
    setRevealed(false);
    cardShownAt.current = Date.now();
  }, []);

  const reveal = useCallback(() => {
    setRevealed(true);
  }, []);

  const rate = useCallback(async (quality: ReviewQuality) => {
    const card = cards[currentIndex];
    if (!card) return;
    const recallSpeedMs = Date.now() - cardShownAt.current;
    await ipc("flashcard_record_review", {
      cardId: card.id,
      quality,
      recallSpeedMs,
    });
    setTotalReviewed((n) => n + 1);
    if (quality !== "again") setCorrectCount((n) => n + 1);
    setRevealed(false);
    setCurrentIndex((i) => i + 1);
    cardShownAt.current = Date.now();
  }, [cards, currentIndex]);

  const current = cards[currentIndex] ?? null;
  const remaining = Math.max(0, cards.length - currentIndex);
  const done = currentIndex >= cards.length && cards.length > 0;

  return {
    cards, currentIndex, current, revealed, done, remaining,
    totalReviewed, correctCount, startTime,
    startReview, reveal, rate, exit: onExit,
  };
}
```

- [ ] **Step 2: Create CardRenderer component**

Renders a card based on its `cardType`. Handles:
- `basic`: front text, divider, back text (when revealed)
- `cloze`: text with `{{c1::hidden}}` replaced by blanks, revealed shows full text
- `vocabulary`: word + reading on front, meaning + example sentence on back
- `typed`: front text + text input field, checks answer on reveal
- `image_occlusion`: placeholder (image cards come in Phase 6)

- [ ] **Step 3: Create RatingButtons component**

Four buttons: Again (red), Hard (orange), Good (green), Easy (blue). Each shows the computed next interval. Keyboard shortcuts: 1/2/3/4. The component accepts `onRate(quality)` callback.

- [ ] **Step 4: Create ImmersiveReview component**

Full-screen review mode. Layout:
- Top bar: ESC button (left), progress "5 / 23" (center), deck name (right)
- Center: CardRenderer
- Bottom: RatingButtons (after reveal), "Show Answer" button (before reveal)
- Footer: [Ask AI] [Edit] [Source note] action buttons
- Progress bar at very bottom

Keyboard shortcuts: Space = reveal/show answer, 1-4 = rate, Escape = exit to dashboard.

- [ ] **Step 5: Create PostSession component**

Shows after `done` is true:
- Cards reviewed count
- Accuracy percentage
- Time spent
- "Back to Dashboard" button

- [ ] **Step 6: Wire into LearnPage**

Update `LearnPage` to manage `mode: "dashboard" | "review" | "post-session"` state. Pass `onStartReview` from dashboard to switch to review mode. Pass `onExit` from review to switch back.

- [ ] **Step 7: Verify full flow in browser**

1. Navigate to `/learn`
2. If no cards exist, create one via the backend (or use Quick Add from Task 6)
3. Click "Start Review"
4. Verify immersive mode opens, card renders
5. Press Space to reveal answer
6. Press 1-4 to rate
7. Complete all cards → verify PostSession shows
8. Press "Back to Dashboard" → verify return to dashboard

---

### Task 6: Quick Add Modal

**Files:**
- Create: `desktop-ui/src/features/learn/components/QuickAdd.tsx`
- Modify: `desktop-ui/src/features/learn/pages/LearnPage.tsx`

- [ ] **Step 1: Create QuickAdd component**

A modal (using Radix Dialog or simple overlay) with:
- Front text input (autofocused)
- Back text input
- Deck selector (dropdown from existing decks, or type new)
- Card type selector (default: Basic)
- "Create" button (Enter key shortcut)
- "Cancel" button (Escape key shortcut)

Uses `ipc("flashcard_create", params)` to create the card.

- [ ] **Step 2: Wire ⌘N shortcut in LearnPage**

Add a `keydown` listener in LearnPage that opens QuickAdd on `Cmd+N` (Mac) / `Ctrl+N` (Windows).

- [ ] **Step 3: Verify in browser**

Press ⌘N on `/learn` → modal opens → type front/back → press Enter → card created → modal closes → deck list updates.

---

### Task 7: Sidebar Due Count Badge

**Files:**
- Modify: `desktop-ui/src/app/layouts/Sidebar.tsx`

- [ ] **Step 1: Add due count badge to Learn sidebar item**

Modify the Sidebar to fetch `flashcard_total_due` and show a small badge on the Learn icon when due > 0:

```tsx
// In Sidebar component, add:
const [dueCount, setDueCount] = useState(0);
useEffect(() => {
  ipc<number>("flashcard_total_due", {})
    .then(setDueCount)
    .catch(() => {});
  const interval = setInterval(() => {
    ipc<number>("flashcard_total_due", {})
      .then(setDueCount)
      .catch(() => {});
  }, 30000); // refresh every 30s
  return () => clearInterval(interval);
}, []);
```

Render a small red dot or number on the Learn icon button:

```tsx
{item.key === "Learn" && dueCount > 0 && (
  <span className="absolute -top-0.5 -right-0.5 w-3.5 h-3.5 rounded-full bg-brand text-[9px] text-white flex items-center justify-center font-medium">
    {dueCount > 99 ? "99" : dueCount}
  </span>
)}
```

The button needs `relative` positioning for the badge to work.

- [ ] **Step 2: Verify in browser**

Navigate to any page. The Learn sidebar icon should show a badge with the due card count (if any exist).

---

### Task 8: Final Verification + Lint

**Files:** None (verification only)

- [ ] **Step 1: Run full workspace Rust tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 new warnings.

- [ ] **Step 3: Run frontend lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Pass with auto-fixes applied.

- [ ] **Step 4: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: Pass (or no tests for new components yet — that's OK for Phase 2).

- [ ] **Step 5: Full manual test in browser**

1. Open `/learn` — see dashboard with stats
2. Press ⌘N — Quick Add modal opens
3. Create a card (front: "Hello", back: "World", deck: "test")
4. See "test" deck appear in deck list with 1 due
5. Click "Start Review" on the test deck
6. See immersive review with the card
7. Press Space → answer reveals
8. Press 3 (Good) → card rated, post-session shows
9. Return to dashboard — due count is now 0 for that deck
10. Check sidebar — badge updates
