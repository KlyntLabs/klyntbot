//! Global memory purge hook.
//!
//! The desktop crate registers its allocator-specific purge function (e.g.
//! `mi_collect`) at startup. Lower-layer crates call [`purge_freed_memory`]
//! after large transient allocations (LanceDB compaction, batch vector ops)
//! to return freed pages to the OS immediately.

use std::sync::OnceLock;

type PurgeFn = fn();

static PURGE_HOOK: OnceLock<PurgeFn> = OnceLock::new();

/// Register the allocator-specific memory purge function.
/// Should be called once at startup by the binary crate.
pub fn set_purge_hook(f: PurgeFn) {
    let _ = PURGE_HOOK.set(f);
}

/// Force the allocator to return freed pages to the OS.
/// No-op if no purge hook has been registered.
pub fn purge_freed_memory() {
    if let Some(f) = PURGE_HOOK.get() {
        f();
    }
}
