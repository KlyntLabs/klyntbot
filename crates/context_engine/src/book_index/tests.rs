#[cfg(test)]
mod integration_tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use common::Result;

    use crate::book_index::*;
    use crate::insight_forge::bookrag_searcher::BookRAGSearcher;
    use crate::insight_forge::domain_searcher::DomainSearcher;
    use crate::operators::OperatorLlm;
    use crate::retrieval_planner::RetrievalPlanner;

    // -- Mock implementations --

    struct MockBookTreeRepo {
        nodes: tokio::sync::Mutex<Vec<TreeNode>>,
    }

    impl MockBookTreeRepo {
        fn new(nodes: Vec<TreeNode>) -> Self {
            Self {
                nodes: tokio::sync::Mutex::new(nodes),
            }
        }
    }

    #[async_trait]
    impl BookTreeRepo for MockBookTreeRepo {
        async fn insert_node(&self, node: &TreeNode) -> Result<()> {
            self.nodes.lock().await.push(node.clone());
            Ok(())
        }
        async fn insert_nodes(&self, nodes: &[TreeNode]) -> Result<()> {
            self.nodes.lock().await.extend(nodes.iter().cloned());
            Ok(())
        }
        async fn get_node(&self, id: &str) -> Result<Option<TreeNode>> {
            Ok(self.nodes.lock().await.iter().find(|n| n.id == id).cloned())
        }
        async fn get_children(&self, parent_id: &str) -> Result<Vec<TreeNode>> {
            Ok(self
                .nodes
                .lock()
                .await
                .iter()
                .filter(|n| n.parent_id.as_deref() == Some(parent_id))
                .cloned()
                .collect())
        }
        async fn get_subtree(&self, node_id: &str) -> Result<Vec<TreeNode>> {
            let nodes = self.nodes.lock().await;
            let mut result = Vec::new();
            let mut stack = vec![node_id.to_string()];
            while let Some(id) = stack.pop() {
                if let Some(node) = nodes.iter().find(|n| n.id == id) {
                    result.push(node.clone());
                    for child in nodes.iter().filter(|n| n.parent_id.as_deref() == Some(&id)) {
                        stack.push(child.id.clone());
                    }
                }
            }
            Ok(result)
        }
        async fn get_root_sections(&self, source_type: &SourceType) -> Result<Vec<TreeNode>> {
            Ok(self
                .nodes
                .lock()
                .await
                .iter()
                .filter(|n| n.parent_id.is_none() && n.source_type.as_str() == source_type.as_str())
                .cloned()
                .collect())
        }
        async fn get_path_to_root(&self, _node_id: &str) -> Result<Vec<TreeNode>> {
            Ok(vec![])
        }
        async fn delete_by_source(
            &self,
            _source_type: &SourceType,
            _source_id: &str,
        ) -> Result<u64> {
            Ok(0)
        }
        async fn search_fts(&self, _query: &str, _limit: usize) -> Result<Vec<TreeNode>> {
            Ok(vec![])
        }
        async fn has_any_nodes(&self) -> Result<bool> {
            Ok(!self.nodes.lock().await.is_empty())
        }
    }

    struct MockGTLinkRepo {
        links: tokio::sync::Mutex<Vec<(String, String)>>,
        tree_repo: Arc<MockBookTreeRepo>,
    }

    impl MockGTLinkRepo {
        fn new(links: Vec<(String, String)>, tree_repo: Arc<MockBookTreeRepo>) -> Self {
            Self {
                links: tokio::sync::Mutex::new(links),
                tree_repo,
            }
        }
    }

    #[async_trait]
    impl GTLinkRepo for MockGTLinkRepo {
        async fn link(&self, entity_id: &str, tree_node_id: &str) -> Result<()> {
            self.links
                .lock()
                .await
                .push((entity_id.to_string(), tree_node_id.to_string()));
            Ok(())
        }
        async fn link_batch(&self, links: &[(String, String)]) -> Result<()> {
            self.links.lock().await.extend(links.iter().cloned());
            Ok(())
        }
        async fn get_linked_nodes(&self, entity_id: &str) -> Result<Vec<TreeNode>> {
            let links = self.links.lock().await;
            let node_ids: Vec<String> = links
                .iter()
                .filter(|(eid, _)| eid == entity_id)
                .map(|(_, nid)| nid.clone())
                .collect();
            let nodes = self.tree_repo.nodes.lock().await;
            Ok(nodes
                .iter()
                .filter(|n| node_ids.contains(&n.id))
                .cloned()
                .collect())
        }
        async fn get_entities_in_subtree(&self, node_id: &str) -> Result<Vec<String>> {
            let links = self.links.lock().await;
            Ok(links
                .iter()
                .filter(|(_, nid)| nid == node_id)
                .map(|(eid, _)| eid.clone())
                .collect())
        }
        async fn delete_by_tree_node(&self, _tree_node_id: &str) -> Result<u64> {
            Ok(0)
        }
        async fn migrate_entity_links(&self, _source: &str, _target: &str) -> Result<()> {
            Ok(())
        }
    }

    struct MockBookEntityRepo;

    #[async_trait]
    impl BookEntityRepo for MockBookEntityRepo {
        async fn find_by_name(&self, query: &str) -> Result<Vec<EntityInfo>> {
            // Return a match for "Project Alpha"
            if query.to_lowercase().contains("project alpha") {
                Ok(vec![EntityInfo {
                    id: "entity-alpha".into(),
                    name: "Project Alpha".into(),
                    entity_type: "Project".into(),
                }])
            } else {
                Ok(vec![])
            }
        }
        async fn get_neighborhood_ids(
            &self,
            _entity_id: &str,
            _depth: u32,
        ) -> Result<Vec<(String, String, f64)>> {
            Ok(vec![])
        }
    }

    struct MockBookEmbedder;

    #[async_trait]
    impl BookEmbedder for MockBookEmbedder {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
            // Return a dummy 384-dim vector
            Ok(vec![0.1; 384])
        }
    }

    struct MockOperatorLlm;

    #[async_trait]
    impl OperatorLlm for MockOperatorLlm {
        async fn complete(&self, _system: &str, prompt: &str) -> Result<String> {
            // For Extract: return entity name
            if prompt.contains("Extract key entity names") {
                Ok("Project Alpha".to_string())
            }
            // For Reduce: synthesize
            else if prompt.contains("Synthesize") || prompt.contains("synthesize") {
                Ok("Project Alpha has a deadline of March 30.".to_string())
            }
            // For any other prompt, return relevant content
            else {
                Ok("Relevant information extracted.".to_string())
            }
        }
    }

    fn make_test_nodes() -> Vec<TreeNode> {
        vec![
            TreeNode {
                id: "section-1".into(),
                parent_id: None,
                node_type: TreeNodeType::Section,
                content: "Project Alpha".into(),
                title: Some("Project Alpha".into()),
                level: 1,
                source_type: SourceType::Note,
                source_id: "note-1".into(),
                position: 0,
                metadata: None,
            },
            TreeNode {
                id: "text-1".into(),
                parent_id: Some("section-1".into()),
                node_type: TreeNodeType::Text,
                content:
                    "Project Alpha has a deadline of March 30. It involves building the new API."
                        .into(),
                title: None,
                level: 2,
                source_type: SourceType::Note,
                source_id: "note-1".into(),
                position: 1,
                metadata: None,
            },
            TreeNode {
                id: "section-2".into(),
                parent_id: None,
                node_type: TreeNodeType::Section,
                content: "Finance Goals".into(),
                title: Some("Finance Goals".into()),
                level: 1,
                source_type: SourceType::Note,
                source_id: "note-2".into(),
                position: 0,
                metadata: None,
            },
        ]
    }

    #[tokio::test]
    async fn single_hop_integration() {
        let nodes = make_test_nodes();
        let tree_repo = Arc::new(MockBookTreeRepo::new(nodes));
        let gt_links = vec![
            ("entity-alpha".to_string(), "section-1".to_string()),
            ("entity-alpha".to_string(), "text-1".to_string()),
        ];
        let gt_link_repo = Arc::new(MockGTLinkRepo::new(gt_links, tree_repo.clone()));

        let book_index = Arc::new(BookIndex::new(
            tree_repo as Arc<dyn BookTreeRepo>,
            Arc::new(MockBookEntityRepo),
            gt_link_repo as Arc<dyn GTLinkRepo>,
            Arc::new(MockBookEmbedder),
        ));
        book_index.refresh_has_content().await.unwrap();
        assert!(book_index.has_content());

        let llm: Arc<dyn OperatorLlm> = Arc::new(MockOperatorLlm);
        let config = BookRetrievalConfig::default();
        let planner = Arc::new(RetrievalPlanner::new(book_index, llm, config));

        let searcher = BookRAGSearcher::new(planner, 50, 10, 5000);

        let results = searcher
            .search("What is the deadline for Project Alpha?", 10)
            .await;

        // Should return relevant nodes for Project Alpha
        assert!(
            !results.is_empty(),
            "SingleHop should return results for entity-linked query"
        );
        // Results should contain content about Project Alpha
        let has_alpha = results.iter().any(|r| r.content.contains("Alpha"));
        assert!(has_alpha, "Results should contain Project Alpha content");
    }

    #[tokio::test]
    async fn passthrough_returns_empty() {
        let tree_repo = Arc::new(MockBookTreeRepo::new(make_test_nodes()));
        let gt_link_repo = Arc::new(MockGTLinkRepo::new(vec![], tree_repo.clone()));
        let book_index = Arc::new(BookIndex::new(
            tree_repo as Arc<dyn BookTreeRepo>,
            Arc::new(MockBookEntityRepo),
            gt_link_repo as Arc<dyn GTLinkRepo>,
            Arc::new(MockBookEmbedder),
        ));
        book_index.refresh_has_content().await.unwrap();

        let llm: Arc<dyn OperatorLlm> = Arc::new(MockOperatorLlm);
        let planner = Arc::new(RetrievalPlanner::new(
            book_index,
            llm,
            BookRetrievalConfig::default(),
        ));
        let searcher = BookRAGSearcher::new(planner, 50, 10, 5000);

        let results = searcher.search("hello", 10).await;
        assert!(
            results.is_empty(),
            "PassThrough queries should return empty"
        );
    }

    #[tokio::test]
    async fn empty_index_returns_empty() {
        let tree_repo = Arc::new(MockBookTreeRepo::new(vec![]));
        let gt_link_repo = Arc::new(MockGTLinkRepo::new(vec![], tree_repo.clone()));
        let book_index = Arc::new(BookIndex::new(
            tree_repo as Arc<dyn BookTreeRepo>,
            Arc::new(MockBookEntityRepo),
            gt_link_repo as Arc<dyn GTLinkRepo>,
            Arc::new(MockBookEmbedder),
        ));
        // Don't refresh — has_content is false by default

        let llm: Arc<dyn OperatorLlm> = Arc::new(MockOperatorLlm);
        let planner = Arc::new(RetrievalPlanner::new(
            book_index,
            llm,
            BookRetrievalConfig::default(),
        ));
        let searcher = BookRAGSearcher::new(planner, 50, 10, 5000);

        let results = searcher
            .search("What is the deadline for Project Alpha?", 10)
            .await;
        assert!(
            results.is_empty(),
            "Empty BookIndex should return no results"
        );
    }

    fn make_section(
        id: &str,
        parent: Option<&str>,
        title: &str,
        source_id: &str,
        level: u32,
    ) -> TreeNode {
        TreeNode {
            id: id.into(),
            parent_id: parent.map(|p| p.into()),
            node_type: TreeNodeType::Section,
            content: title.into(),
            title: Some(title.into()),
            level,
            source_type: SourceType::Note,
            source_id: source_id.into(),
            position: 0,
            metadata: None,
        }
    }

    fn make_text(id: &str, parent: &str, content: &str, source_id: &str, level: u32) -> TreeNode {
        TreeNode {
            id: id.into(),
            parent_id: Some(parent.into()),
            node_type: TreeNodeType::Text,
            content: content.into(),
            title: None,
            level,
            source_type: SourceType::Note,
            source_id: source_id.into(),
            position: 1,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn multi_hop_integration() {
        let nodes = vec![
            make_section("s1", None, "Project Alpha", "note-1", 1),
            make_text(
                "t1",
                "s1",
                "Project Alpha uses Rust and has deadline March 30.",
                "note-1",
                2,
            ),
            make_section("s2", None, "Finance Goals", "note-2", 1),
            make_text(
                "t2",
                "s2",
                "Budget for Project Alpha is $15,000.",
                "note-2",
                2,
            ),
        ];
        let tree_repo = Arc::new(MockBookTreeRepo::new(nodes));
        let gt_links = vec![
            ("entity-alpha".to_string(), "s1".to_string()),
            ("entity-alpha".to_string(), "t1".to_string()),
            ("entity-alpha".to_string(), "t2".to_string()),
        ];
        let gt_link_repo = Arc::new(MockGTLinkRepo::new(gt_links, tree_repo.clone()));

        let book_index = Arc::new(BookIndex::new(
            tree_repo as Arc<dyn BookTreeRepo>,
            Arc::new(MockBookEntityRepo),
            gt_link_repo as Arc<dyn GTLinkRepo>,
            Arc::new(MockBookEmbedder),
        ));
        book_index.refresh_has_content().await.unwrap();

        let llm: Arc<dyn OperatorLlm> = Arc::new(MockOperatorLlm);
        let planner = Arc::new(RetrievalPlanner::new(
            book_index,
            llm,
            BookRetrievalConfig::default(),
        ));
        let searcher = BookRAGSearcher::new(planner, 50, 10, 5000);

        // "relate" keyword triggers MultiHop in heuristic
        let results = searcher
            .search("How does Project Alpha relate to my finance goals?", 10)
            .await;
        assert!(
            !results.is_empty(),
            "MultiHop should return results from linked notes"
        );
    }

    #[tokio::test]
    async fn global_aggregation_integration() {
        let nodes = vec![
            make_section("s1", None, "Note 1", "note-1", 1),
            make_text("t1", "s1", "Task A is overdue.", "note-1", 2),
            make_section("s2", None, "Note 2", "note-2", 1),
            make_text("t2", "s2", "Task B is overdue.", "note-2", 2),
        ];
        let tree_repo = Arc::new(MockBookTreeRepo::new(nodes));
        let gt_link_repo = Arc::new(MockGTLinkRepo::new(vec![], tree_repo.clone()));

        let book_index = Arc::new(BookIndex::new(
            tree_repo as Arc<dyn BookTreeRepo>,
            Arc::new(MockBookEntityRepo),
            gt_link_repo as Arc<dyn GTLinkRepo>,
            Arc::new(MockBookEmbedder),
        ));
        book_index.refresh_has_content().await.unwrap();

        let llm: Arc<dyn OperatorLlm> = Arc::new(MockOperatorLlm);
        let planner = Arc::new(RetrievalPlanner::new(
            book_index,
            llm,
            BookRetrievalConfig::default(),
        ));
        let searcher = BookRAGSearcher::new(planner, 50, 10, 5000);

        // "across all" triggers GlobalAggregation
        let _results = searcher
            .search("How many tasks are overdue across all projects?", 10)
            .await;
        // May be empty (mock FTS returns nothing) but should NOT crash
        // The important thing is the pipeline executed without error
    }

    #[tokio::test]
    async fn empty_bookrag_does_not_crash() {
        // Regression: BookRAGSearcher with empty index should return empty, not panic
        let empty_tree = Arc::new(MockBookTreeRepo::new(vec![]));
        let empty_gt = Arc::new(MockGTLinkRepo::new(vec![], empty_tree.clone()));
        let book_index = Arc::new(BookIndex::new(
            empty_tree as Arc<dyn BookTreeRepo>,
            Arc::new(MockBookEntityRepo),
            empty_gt as Arc<dyn GTLinkRepo>,
            Arc::new(MockBookEmbedder),
        ));
        // has_content is false — searcher should short-circuit

        let llm: Arc<dyn OperatorLlm> = Arc::new(MockOperatorLlm);
        let planner = Arc::new(RetrievalPlanner::new(
            book_index,
            llm,
            BookRetrievalConfig::default(),
        ));
        let searcher = BookRAGSearcher::new(planner, 50, 10, 5000);

        let results = searcher
            .search(
                "Tell me about something complex and interesting across many domains",
                10,
            )
            .await;
        assert!(
            results.is_empty(),
            "Empty BookIndex should gracefully return empty"
        );
    }
}
