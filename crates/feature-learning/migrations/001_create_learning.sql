-- Learning feature does not own its own tables in v3.
-- Tables `knowledge_atoms`, `flashcards`, `flashcard_reviews`, `fsrs_parameters`
-- live in the cognitive crate's migration set (cognitive/migrations/001_cognitive_tables.sql).
-- This file exists so FeaturePackage::migrations() returns a non-empty vector for
-- migration tracking parity with other features.
SELECT 1;
