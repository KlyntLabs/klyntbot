//! Tests for `Page<T>` cursor-based pagination (AC 1.7).

use tools_core::Page;

// ============================================================
// AC 1.7: Page::empty() returns correct state
// ============================================================

#[test]
fn test_page_empty_has_no_items() {
    let page = Page::<String>::empty();
    assert!(page.items.is_empty());
}

#[test]
fn test_page_empty_has_no_cursor() {
    let page = Page::<String>::empty();
    assert!(page.cursor.is_none());
}

#[test]
fn test_page_empty_has_more_is_false() {
    let page = Page::<String>::empty();
    assert!(!page.has_more);
}

// ============================================================
// AC 1.7: Page::single_page() returns correct state
// ============================================================

#[test]
fn test_page_single_page_has_items() {
    let page = Page::single_page(vec!["a".to_string(), "b".to_string()]);
    assert_eq!(page.items, vec!["a", "b"]);
}

#[test]
fn test_page_single_page_has_no_cursor() {
    let page = Page::single_page(vec!["a".to_string()]);
    assert!(page.cursor.is_none());
}

#[test]
fn test_page_single_page_has_more_is_false() {
    let page = Page::single_page(vec!["a".to_string()]);
    assert!(!page.has_more);
}

#[test]
fn test_page_single_page_with_empty_vec() {
    let page = Page::<String>::single_page(vec![]);
    assert!(page.items.is_empty());
    assert!(!page.has_more);
}

// ============================================================
// AC 1.7: Page::new() with cursor
// ============================================================

#[test]
fn test_page_new_with_cursor() {
    let page = Page::new(
        vec!["item1".to_string()],
        Some("cursor_abc".to_string()),
        true,
    );
    assert_eq!(page.cursor, Some("cursor_abc".to_string()));
    assert!(page.has_more);
}

#[test]
fn test_page_new_without_cursor() {
    let page = Page::new(vec!["item1".to_string()], None, false);
    assert!(page.cursor.is_none());
    assert!(!page.has_more);
}

#[test]
fn test_page_new_preserves_items() {
    let page = Page::new(vec![1, 2, 3], None, false);
    assert_eq!(page.items, vec![1, 2, 3]);
}

// ============================================================
// AC 1.7: Serialization skips None cursor
// ============================================================

#[test]
fn test_page_serialization_skips_none_cursor() {
    let page = Page::<String>::empty();
    let json = serde_json::to_value(&page).unwrap();
    assert!(!json.as_object().unwrap().contains_key("cursor"));
}

#[test]
fn test_page_serialization_includes_some_cursor() {
    let page = Page::new(vec!["x".to_string()], Some("abc".to_string()), true);
    let json = serde_json::to_value(&page).unwrap();
    assert_eq!(json["cursor"], "abc");
}

#[test]
fn test_page_serialization_includes_has_more() {
    let page = Page::new(vec!["x".to_string()], None, true);
    let json = serde_json::to_value(&page).unwrap();
    assert!(json.get("has_more").is_some());
    assert_eq!(json["has_more"], true);
}

#[test]
fn test_page_serialization_includes_items() {
    let page = Page::single_page(vec!["a".to_string(), "b".to_string()]);
    let json = serde_json::to_value(&page).unwrap();
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0], "a");
    assert_eq!(items[1], "b");
}
