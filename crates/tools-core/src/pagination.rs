//! Cursor-based pagination for tool list operations.

use serde::Serialize;

/// A page of results with an opaque cursor for fetching the next page.
#[derive(Debug, Clone, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    pub has_more: bool,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, cursor: Option<String>, has_more: bool) -> Self {
        Self {
            items,
            cursor,
            has_more,
        }
    }

    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            cursor: None,
            has_more: false,
        }
    }

    pub fn single_page(items: Vec<T>) -> Self {
        Self {
            items,
            cursor: None,
            has_more: false,
        }
    }
}
