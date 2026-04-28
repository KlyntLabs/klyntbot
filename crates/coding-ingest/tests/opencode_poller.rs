//! Opencode SQLite poller smoke test.

use coding_ingest::adapters::opencode::OpencodeAdapter;
use coding_ingest::adapters::IngestAdapter;

#[test]
fn opencode_source_name() {
    assert_eq!(OpencodeAdapter.source_name(), "opencode");
}

#[test]
fn opencode_is_poll_only_no_hook_parse() {
    // Opencode is poll-only: the hook entry point never produces events.
    // Calling `parse()` for any event name must return Ok(None) — there is no
    // tier-1 hook surface for opencode (the daemon's `OpencodePoller` task is
    // what drives ingestion).
    let adapter = OpencodeAdapter;
    let result = adapter
        .parse("SessionStart", b"{}")
        .expect("parse should not error for opencode (poll-only)");
    assert!(
        result.is_none(),
        "opencode hook surface should always return None"
    );
}
