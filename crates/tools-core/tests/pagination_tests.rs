//! Tests for `Page<T>` cursor-based pagination (AC 1.7).
//!
//! Page<T> provides:
//! - `empty()` → no items, no cursor, has_more = false
//! - `single_page(items)` → items, no cursor, has_more = false
//! - `new(items, cursor, has_more)` → full construction
//! - Serialization that skips `cursor` when `None`
//!
//! Dev: implement these tests via TDD alongside the implementation in
//! `crates/tools-core/src/pagination.rs`.

// NOTE: Adjust imports once tools-core/src/pagination.rs is finalized.
//
// Expected imports:
//   use tools_core::Page;
//   use serde_json;

// ============================================================
// AC 1.7: Page::empty() returns correct state
// ============================================================

#[test]
fn test_page_empty_has_no_items() {
    // Verifies: Page::<String>::empty().items is empty.
    // AC 1.7
    todo!()
}

#[test]
fn test_page_empty_has_no_cursor() {
    // Verifies: Page::<String>::empty().cursor is None.
    // AC 1.7
    todo!()
}

#[test]
fn test_page_empty_has_more_is_false() {
    // Verifies: Page::<String>::empty().has_more is false.
    // AC 1.7
    todo!()
}

// ============================================================
// AC 1.7: Page::single_page() returns correct state
// ============================================================

#[test]
fn test_page_single_page_has_items() {
    // Verifies: Page::single_page(vec!["a", "b"]).items == vec!["a", "b"].
    // AC 1.7
    todo!()
}

#[test]
fn test_page_single_page_has_no_cursor() {
    // Verifies: Page::single_page(vec!["a"]).cursor is None.
    // AC 1.7
    todo!()
}

#[test]
fn test_page_single_page_has_more_is_false() {
    // Verifies: Page::single_page(vec!["a"]).has_more is false.
    // AC 1.7
    todo!()
}

#[test]
fn test_page_single_page_with_empty_vec() {
    // Verifies: Page::single_page(vec![]).items is empty, has_more is false.
    // AC 1.7
    todo!()
}

// ============================================================
// AC 1.7: Page::new() with cursor
// ============================================================

#[test]
fn test_page_new_with_cursor() {
    // Verifies: Page::new(items, Some("cursor_abc"), true)
    //   .cursor == Some("cursor_abc")
    //   .has_more == true.
    // AC 1.7
    todo!()
}

#[test]
fn test_page_new_without_cursor() {
    // Verifies: Page::new(items, None, false)
    //   .cursor == None
    //   .has_more == false.
    // AC 1.7
    todo!()
}

#[test]
fn test_page_new_preserves_items() {
    // Verifies: Page::new(vec![1, 2, 3], None, false).items == vec![1, 2, 3].
    // AC 1.7
    todo!()
}

// ============================================================
// AC 1.7: Serialization skips None cursor
// ============================================================

#[test]
fn test_page_serialization_skips_none_cursor() {
    // Verifies: When cursor is None, serialized JSON does NOT contain a "cursor" key.
    // Page::empty::<String>() serialized → {"items":[],"has_more":false}
    // (no "cursor" field).
    // AC 1.7
    todo!()
}

#[test]
fn test_page_serialization_includes_some_cursor() {
    // Verifies: When cursor is Some("abc"), serialized JSON DOES contain "cursor": "abc".
    // AC 1.7
    todo!()
}

#[test]
fn test_page_serialization_includes_has_more() {
    // Verifies: Serialized JSON always includes "has_more" field.
    // AC 1.7
    todo!()
}

#[test]
fn test_page_serialization_includes_items() {
    // Verifies: Serialized JSON includes "items" as an array.
    // AC 1.7
    todo!()
}
