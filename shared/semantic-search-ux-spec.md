# Semantic Search — UX Specification

**Author**: UX Designer Agent
**Sprint**: 5
**Date**: 2026-02-15
**Status**: Ready for Review

---

## 1. Design Principles

1. **Non-breaking**: Keyword search stays default. Semantic/hybrid are opt-in flags.
2. **Progressive disclosure**: Basic usage is simple; power options (threshold, limit) are discoverable via `--help`.
3. **Consistent with existing UI**: Reuse the same color scheme, layout, and status indicators from `todo list` and `todo search`.
4. **Graceful degradation**: If semantic search is unavailable, fall back to keyword search with a clear message.
5. **No emoji in output**: Follow existing codebase convention — use Unicode indicators (✓, ●, ✗) only where already established.

---

## 2. CLI Command Structure

### 2.1 New Flags on `klyntbot todo search`

```
klyntbot todo search [FLAGS] [OPTIONS] <query>

FLAGS:
    -s, --semantic           Use semantic search (find related concepts)
        --hybrid             Combine keyword + semantic search
    -a, --include-attachments  Also search attachment content (existing)
        --help               Print help information

OPTIONS:
    -t, --threshold <FLOAT>  Similarity threshold (0.0–1.0, default: 0.5)
    -l, --limit <N>          Maximum results (1–100, default: 10)
```

**Flag decisions:**
- `--semantic` gets short flag `-s` (high-frequency flag, ergonomic)
- `--hybrid` gets NO short flag (avoid `-h` conflict with `--help`)
- `--threshold` gets short flag `-t`
- `--limit` gets short flag `-l`
- `--semantic` and `--hybrid` are mutually exclusive (clap `conflicts_with`)

### 2.2 Usage Examples (for help text)

```
EXAMPLES:
    klyntbot todo search "auth bug"                           # keyword (default)
    klyntbot todo search --semantic "authentication security" # semantic
    klyntbot todo search --hybrid "login issue"               # both combined
    klyntbot todo search -s "auth" --threshold 0.7            # higher precision
    klyntbot todo search -s "auth" --limit 5                  # fewer results
```

---

## 3. Output Format Design

### 3.1 Keyword Search (unchanged)

Current behavior preserved exactly:

```
3 task(s) matching 'auth bug':

abc123 ● Fix authentication bug  P3
def456   Update auth token expiry  P2
ghi789   Add OAuth support  P4
```

Colors: ID in cyan (`TOOL`), status icon (✓ green / ● orange), title in bold, priority rendered per existing `render_priority`.

### 3.2 Semantic Search Output

Header changes to indicate search mode. Each result shows a similarity score using a compact `sim:` label, right-aligned after the priority.

```
5 task(s) matching 'authentication security' (semantic, threshold: 0.5):

abc123 ● Fix authentication bug  P3  sim: 0.92
def456   Login system refactor  P2  sim: 0.87
ghi789   Security hardening  P4  sim: 0.78
jkl012   Update auth token expiry  P2  sim: 0.65
mno345   Add OAuth support  P4  sim: 0.52
```

**Score display rationale (answering BA Q6):**
- **Raw float (0.92)** — chosen over percentage or qualitative labels because:
  - It's precise and honest (no false sense of certainty from "92% match")
  - It aligns with the `--threshold` flag which uses the same 0.0–1.0 scale
  - Developers (primary audience) prefer raw data they can reason about
  - Qualitative labels ("strong match") hide the threshold boundary
- Format: `sim: 0.XX` — always 2 decimal places, padded (e.g., `0.50` not `0.5`)
- Color: DIM gray for `sim:` label, HIGHLIGHT orange for the score value when >= 0.8, default otherwise

### 3.3 Hybrid Search Output

Header indicates both modes. Results show match source.

```
6 task(s) matching 'login' (hybrid: keyword + semantic):

abc123 ● Fix login bug  P3  keyword + semantic (0.94)
def456   Login system refactor  P2  semantic (0.87)
ghi789   Security hardening  P4  semantic (0.78)
jkl012   Update auth token expiry  P2  keyword + semantic (0.72)
mno345   Add OAuth support  P4  semantic (0.52)
pqr678   Auth middleware cleanup  P1  keyword
```

**Match source indicators:**
- `keyword + semantic (0.XX)` — matched both ways, show semantic score
- `semantic (0.XX)` — semantic match only, show score
- `keyword` — keyword match only, no score (it's a binary match)
- Color: match source in DIM gray

### 3.4 Partial Embedding Coverage (EC-4)

When some tasks lack embeddings, append a note after results:

```
5 task(s) matching 'auth' (semantic, threshold: 0.5):

[... results ...]

Note: 12 of 50 tasks have no embeddings (not included in semantic results).
```

Color: entire note line in DIM gray.

---

## 4. Loading States

### 4.1 Model Download (first-time, ~23 MB)

Use the existing `Spinner` from `common::utils::terminal::spinners` for indeterminate progress, but switch to a progress bar for the download since we know the total size.

```
Downloading embedding model (23 MB)...
[████████████████████--------] 65%
```

**Implementation note**: Use the `indicatif` crate for the download progress bar (it's the Rust standard for progress bars). The Spinner is too simple for determinate progress.

If download fails:
```
Failed to download embedding model: {error}
Semantic search is unavailable. Use keyword search instead.
```

### 4.2 Backfill (embedding generation for existing tasks)

When the user triggers semantic search and no embeddings exist, or runs an explicit backfill:

```
Generating embeddings for 247 tasks...
[████████████████████████████] 100% (247/247)

5 task(s) matching 'auth' (semantic, threshold: 0.5):
[... results ...]
```

Progress bar format: `[bar] XX% (current/total)` using `indicatif`.

### 4.3 Single-task Embedding (on `todo add` / `todo update`)

No visible feedback needed — embedding generation is < 50ms per task. The existing command latency absorbs this. If it fails, silently fall back (logged as warning internally).

---

## 5. Empty & Error States

### 5.1 No Results — Keyword (unchanged)

```
No tasks found matching 'xyz'.
```

### 5.2 No Results — Semantic

```
No tasks found matching 'xyz' (semantic search, threshold: 0.5).

Suggestions:
  Lower the threshold:  klyntbot todo search -s "xyz" --threshold 0.3
  Try keyword search:   klyntbot todo search "xyz"
```

Color: "Suggestions:" in DIM, command examples in default (readable).

### 5.3 No Results — Hybrid

```
No tasks found matching 'xyz' (hybrid search, threshold: 0.5).

Suggestions:
  Lower the threshold:  klyntbot todo search --hybrid "xyz" --threshold 0.3
  Try keyword search:   klyntbot todo search "xyz"
```

### 5.4 No Embeddings Exist (EC-3)

```
No task embeddings found. Semantic search requires embeddings.

To generate embeddings for existing tasks:
  klyntbot todo search -s "your query"  (auto-generates on first run)
```

### 5.5 Empty Query (EC-1, EC-2)

```
Error: Search query cannot be empty.
```

Color: "Error:" in RED (`ERROR`), message in default.

### 5.6 Query Too Long

```
Error: Query too long (max 1000 characters).
```

### 5.7 Semantic Search Unavailable (EC-12)

When the embedding model fails to initialize:

```
Semantic search unavailable: {error}
Falling back to keyword search...

3 task(s) matching 'auth':
[... keyword results ...]
```

Color: first line in WARNING yellow. "Falling back..." in DIM.

**Key UX decision**: Auto-fallback to keyword search rather than failing entirely. The user still gets results.

### 5.8 Invalid Threshold

```
Error: Threshold must be between 0.0 and 1.0 (got: 1.5).
```

### 5.9 Invalid Limit

```
Error: Limit must be between 1 and 100 (got: 0).
```

---

## 6. Help Text

Full `--help` output for the search subcommand:

```
Search for tasks by keyword or semantic similarity

Usage: klyntbot todo search [FLAGS] [OPTIONS] <query>

Arguments:
  <query>  Search query text

Flags:
  -s, --semantic              Use semantic search (finds related concepts, synonyms)
      --hybrid                Combine keyword and semantic search
  -a, --include-attachments   Also search attachment content
  -h, --help                  Print help

Options:
  -t, --threshold <FLOAT>     Similarity threshold for semantic/hybrid search [default: 0.5]
  -l, --limit <N>             Maximum number of results [default: 10]

Examples:
  klyntbot todo search "auth bug"                 Keyword search (default)
  klyntbot todo search -s "authentication"         Semantic search
  klyntbot todo search --hybrid "login issue"      Keyword + semantic
  klyntbot todo search -s "auth" -t 0.7            Higher precision
  klyntbot todo search -s "deploy" -l 5            Limit results
```

---

## 7. Color & Typography Reference

All output uses the existing color scheme from `common::utils::terminal::colors`:

| Element | Color Constant | Hex Equivalent |
|---------|---------------|----------------|
| Task ID | `TOOL` (cyan) | `\x1b[36m` |
| Title | `BOLD` | `\x1b[1m` |
| Priority | `render_priority()` | varies by level |
| Status done | `SUCCESS` (green) | `\x1b[32m` |
| Status active | `BRAND` (orange) | `\x1b[38;5;208m` |
| Similarity score (>= 0.8) | `HIGHLIGHT` (bright orange) | `\x1b[38;5;214m` |
| Similarity score (< 0.8) | default | — |
| `sim:` label | `DIM` (gray) | `\x1b[90m` |
| Match source | `DIM` (gray) | `\x1b[90m` |
| Header note (mode, threshold) | `DIM` (gray) | `\x1b[90m` |
| Warning message | `WARNING` (yellow) | `\x1b[33m` |
| Error label | `ERROR` (red) | `\x1b[31m` |
| Suggestions | `DIM` (gray) | `\x1b[90m` |
| Progress bar | `indicatif` default | — |

**NO_COLOR compliance**: All color output respects the existing `colors_enabled()` check. When `NO_COLOR` is set, all ANSI codes are stripped.

---

## 8. Accessibility

1. **NO_COLOR support**: Already built into the color system. All output degrades to plain text.
2. **Screen reader friendly**: No decorative Unicode beyond existing ✓/● indicators. Scores are numeric, not icon-based.
3. **Consistent layout**: Same left-aligned format as existing search output. No columns that break on narrow terminals.
4. **Actionable suggestions**: Empty/error states always include a concrete next command to try.
5. **Threshold in header**: Always shown in semantic/hybrid output headers so the user knows what filter is active.

---

## 9. State Machine — Search Flow

```
User runs: klyntbot todo search [--semantic|--hybrid] "query"
  │
  ├─ Validate input (empty query? too long? bad threshold?)
  │   └─ Error → print error message, exit 1
  │
  ├─ If --semantic or --hybrid:
  │   ├─ Try to initialize embedding engine
  │   │   ├─ Model not downloaded → download with progress bar → continue
  │   │   └─ Init failed → if --hybrid, warn + keyword fallback
  │   │                   → if --semantic, error message + exit 1
  │   │
  │   ├─ Check embedding coverage
  │   │   ├─ Zero embeddings → auto-backfill with progress bar → continue
  │   │   └─ Partial embeddings → continue (note shown after results)
  │   │
  │   ├─ Generate query embedding
  │   ├─ Compute cosine similarity against all stored embeddings
  │   ├─ Filter by threshold
  │   │
  │   ├─ If --hybrid: also run keyword search, merge via RRF
  │   │
  │   └─ Display results (format per §3.2 or §3.3)
  │       └─ Zero results → empty state (§5.2 or §5.3)
  │
  └─ If keyword only (no flags):
      └─ Existing behavior, unchanged (§3.1)
```

---

## 10. Decisions Summary

| Question | Decision | Rationale |
|----------|----------|-----------|
| BA Q6: Score display format | Raw float `0.XX` | Precise, matches `--threshold` scale, developer-friendly |
| BA: Threshold cutoff shown? | Yes, in header | Transparency — user knows why results may be excluded |
| BA: "Partial embeddings" note? | Yes, after results | Awareness without blocking the search |
| BA: Backfill suggestion | Auto-backfill on first semantic search | Zero-friction; user doesn't need to know about backfill |
| BA: Model download indicator | `indicatif` progress bar | Determinate progress (known size) is less stressful than spinner |
| BA: `--hybrid` short flag | No short flag | Avoids `-h`/`--help` conflict |
| Team lead: `-s` short flag | Confirmed `-s` for `--semantic` | Ergonomic, no conflicts |
| Fallback behavior | `--hybrid` auto-falls back to keyword; `--semantic` does NOT | Hybrid is inherently mixed-mode; pure semantic failing should be explicit |
| EC-12: Model failure | Yellow warning + fallback (hybrid) or error (semantic) | Different expectations per mode |
| EC-4: Partial embeddings | Note at bottom, not inline | Keep results clean; meta-info secondary |

---

## 11. Implementation Notes for Frontend Developer

1. **File to modify**: `crates/cli/src/commands.rs` — add `semantic`, `hybrid`, `threshold`, `limit` fields to the `Search` variant of `TodoCommands`.

2. **File to modify**: `crates/cli/src/todo/search.rs` — branch on flags. For keyword-only, preserve existing `handle_search` logic. For semantic/hybrid, call the tool layer.

3. **New dependency suggestion**: `indicatif = "0.17"` in `cli/Cargo.toml` for progress bars. The existing `Spinner` in common is insufficient for determinate progress.

4. **Score formatting helper**: Create a small helper `format_score(score: f32) -> String` that returns `"0.XX"` (always 2 decimal places). Use `format!("{:.2}", score)`.

5. **Color logic for scores**:
   ```rust
   if score >= 0.8 {
       colorize(&format_score(score), HIGHLIGHT)
   } else {
       format_score(score)
   }
   ```

6. **Header format string pattern**:
   - Keyword: `"{count} task(s) matching '{query}':"` (unchanged)
   - Semantic: `"{count} task(s) matching '{query}' (semantic, threshold: {threshold}):"`
   - Hybrid: `"{count} task(s) matching '{query}' (hybrid: keyword + semantic):"`
   - The parenthetical portion should be in DIM gray.

7. **Match source rendering (hybrid only)**: After priority, render one of:
   - `keyword + semantic ({score})` — both matched
   - `semantic ({score})` — semantic only
   - `keyword` — keyword only
