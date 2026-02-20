//! Tests for `ConfigPersistence` trait (AC 1.9).

use async_trait::async_trait;
use common::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tools_core::ConfigPersistence;

// --- Mock implementation ---

struct MockConfigPersistence {
    sections: Mutex<HashMap<String, Value>>,
}

impl MockConfigPersistence {
    fn new() -> Self {
        Self {
            sections: Mutex::new(HashMap::new()),
        }
    }

    fn with_section(key: &str, value: Value) -> Self {
        let mut map = HashMap::new();
        map.insert(key.to_string(), value);
        Self {
            sections: Mutex::new(map),
        }
    }
}

#[async_trait]
impl ConfigPersistence for MockConfigPersistence {
    async fn load_section(&self, key: &str) -> Result<Value> {
        let sections = self.sections.lock().unwrap();
        Ok(sections.get(key).cloned().unwrap_or(Value::Null))
    }

    async fn save_section(&self, key: &str, value: Value) -> Result<()> {
        let mut sections = self.sections.lock().unwrap();
        sections.insert(key.to_string(), value);
        Ok(())
    }
}

// ============================================================
// AC 1.9: Trait is object-safe (can be Arc<dyn ConfigPersistence>)
// ============================================================

#[test]
fn test_config_persistence_is_object_safe() {
    let _: Arc<dyn ConfigPersistence> = Arc::new(MockConfigPersistence::new());
}

// ============================================================
// AC 1.9: Mock implementation works — load_section
// ============================================================

#[tokio::test]
async fn test_mock_load_section_returns_stored_value() {
    let mock = MockConfigPersistence::with_section("todo", json!({"enabled": true}));
    let result = mock.load_section("todo").await.unwrap();
    assert_eq!(result, json!({"enabled": true}));
}

#[tokio::test]
async fn test_mock_load_section_missing_key_returns_error_or_default() {
    let mock = MockConfigPersistence::new();
    let result = mock.load_section("nonexistent").await.unwrap();
    assert_eq!(result, Value::Null);
}

// ============================================================
// AC 1.9: Mock implementation works — save_section
// ============================================================

#[tokio::test]
async fn test_mock_save_section_persists_value() {
    let mock = MockConfigPersistence::new();
    mock.save_section("todo", json!({"enabled": false}))
        .await
        .unwrap();
    let result = mock.load_section("todo").await.unwrap();
    assert_eq!(result, json!({"enabled": false}));
}

#[tokio::test]
async fn test_mock_save_section_overwrites_existing() {
    let mock = MockConfigPersistence::with_section("todo", json!({"enabled": true}));
    mock.save_section("todo", json!({"enabled": false}))
        .await
        .unwrap();
    let result = mock.load_section("todo").await.unwrap();
    assert_eq!(result, json!({"enabled": false}));
}

// ============================================================
// AC 1.9: Trait requires Send + Sync
// ============================================================

#[test]
fn test_config_persistence_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<dyn ConfigPersistence>>();
}

// ============================================================
// AC 1.9: Multiple sections independent
// ============================================================

#[tokio::test]
async fn test_mock_multiple_sections_independent() {
    let mock = MockConfigPersistence::new();
    mock.save_section("todo", json!({"maxSlots": 5}))
        .await
        .unwrap();
    mock.save_section("finance", json!({"currency": "USD"}))
        .await
        .unwrap();

    let todo = mock.load_section("todo").await.unwrap();
    let finance = mock.load_section("finance").await.unwrap();

    assert_eq!(todo, json!({"maxSlots": 5}));
    assert_eq!(finance, json!({"currency": "USD"}));
}
