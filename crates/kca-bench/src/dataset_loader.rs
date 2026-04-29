//! Helpers to fetch upstream sources and transform them. For most CI runs we ship
//! committed JSONL files; the loader is for periodic refresh.

pub async fn refresh_all_fixtures(_target_dir: &std::path::Path) -> common::Result<()> {
    // Implementation: fetch upstream, convert, write JSONL. Optional.
    Ok(())
}
