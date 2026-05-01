# klynt-protocol — Vendor Provenance

**Adapted from:** `codex-rs/protocol/` (upstream commit pending — pinned in Plan 2)
**License:** Apache-2.0
**Adaptation script:** `scripts/adapt_codex_vendor.sh`

**Renames applied (planned for Plan 2):**
- `codex_*` → `klynt_*` (modules)
- `CodexEvent` → `KlyntEvent` (types)
- `~/.codex/` → `~/.klyntbot/` (paths)
- `CODEX_API_KEY` → `KLYNT_API_KEY` (env vars)

**Phase 1 (Plan 1):** Empty skeleton; only the package metadata exists.
**Phase 1 (Plan 2):** Vendored sources land via the adapt script.

# Vendoring notes

Adapted from codex-rs/protocol/.
Wire-protocol types (TCP observer) deleted per spec §3.
Minimal subset: Op, Submission, SubmissionResult, CodingTraceEvent, ProtocolError.
