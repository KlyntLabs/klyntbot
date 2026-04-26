// Compatibility shim: all Tauri API calls have been moved to `src/api/`.
// This file re-exports the entire public API so existing imports continue to work.
// Prefer importing from `@/api` or `@services/tauri` in new code.
export * from "@/api";
