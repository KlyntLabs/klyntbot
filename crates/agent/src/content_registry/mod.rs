//! Content registry: multi-source documentation and skills.
//!
//! Loads content from builtin, local, and remote sources. Provides
//! keyword search across all loaded entries.

pub mod loader;
pub mod search;
pub mod types;

pub use types::*;

use config::ContentConfig;

/// Registry of documentation and skill entries from multiple sources.
pub struct ContentRegistry {
    docs: Vec<DocEntry>,
    skills: Vec<SkillEntry>,
}

impl ContentRegistry {
    /// Create an empty registry.
    pub fn empty() -> Self {
        Self {
            docs: Vec::new(),
            skills: Vec::new(),
        }
    }

    /// Load content from all configured sources (synchronous).
    pub fn load_sync(config: &ContentConfig) -> common::Result<Self> {
        loader::load_all(config)
    }

    /// Add a documentation entry.
    pub fn add_doc(&mut self, doc: DocEntry) {
        self.docs.push(doc);
    }

    /// Add a skill entry.
    pub fn add_skill(&mut self, skill: SkillEntry) {
        self.skills.push(skill);
    }

    /// Get all documentation entries.
    pub fn docs(&self) -> &[DocEntry] {
        &self.docs
    }

    /// Get all skill entries.
    pub fn skills(&self) -> &[SkillEntry] {
        &self.skills
    }

    /// Search across all content by keyword relevance.
    pub fn search(&self, query: &str, limit: usize) -> Vec<ContentSearchResult> {
        search::search_content(&self.docs, &self.skills, query, limit)
    }

    /// Look up a content entry by ID (checks docs first, then skills).
    pub fn get(&self, id: &str) -> Option<ContentEntry> {
        self.docs
            .iter()
            .find(|d| d.id == id)
            .map(|d| ContentEntry::Doc(d.clone()))
            .or_else(|| {
                self.skills
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| ContentEntry::Skill(s.clone()))
            })
    }

    /// Total number of entries (docs + skills).
    pub fn len(&self) -> usize {
        self.docs.len() + self.skills.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty() && self.skills.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = ContentRegistry::empty();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_add_and_get_doc() {
        let mut registry = ContentRegistry::empty();
        registry.add_doc(DocEntry {
            id: "test/doc".into(),
            name: "Test Doc".into(),
            description: "A test document".into(),
            source: "test".into(),
            tags: vec![],
            content_source: "test".into(),
            languages: vec![],
        });

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let entry = registry.get("test/doc");
        assert!(entry.is_some());
        if let Some(ContentEntry::Doc(doc)) = entry {
            assert_eq!(doc.name, "Test Doc");
        } else {
            panic!("Expected Doc entry");
        }
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let registry = ContentRegistry::empty();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_search_docs() {
        let mut registry = ContentRegistry::empty();
        registry.add_doc(DocEntry {
            id: "stripe/api".into(),
            name: "Stripe API".into(),
            description: "Payment processing REST API".into(),
            source: "community".into(),
            tags: vec!["payment".into(), "api".into()],
            content_source: "test".into(),
            languages: vec![],
        });
        registry.add_doc(DocEntry {
            id: "react/hooks".into(),
            name: "React Hooks".into(),
            description: "React state management hooks".into(),
            source: "community".into(),
            tags: vec!["react".into(), "frontend".into()],
            content_source: "test".into(),
            languages: vec![],
        });

        let results = registry.search("payment API", 10);
        assert!(!results.is_empty());
        // Stripe should rank first for "payment API"
        if let ContentEntry::Doc(doc) = &results[0].entry {
            assert_eq!(doc.id, "stripe/api");
        } else {
            panic!("Expected Doc entry");
        }
    }

    #[test]
    fn test_load_sync_empty_config() {
        let config = ContentConfig::default();
        let registry = ContentRegistry::load_sync(&config).unwrap();
        assert!(registry.is_empty());
    }
}
