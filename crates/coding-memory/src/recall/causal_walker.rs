//! Bounded BFS over `memory_causal_edges`. Cycle-safe via visited-set.

use crate::causal::CausalEdgeRepo;
use crate::recall::CausalTraceResponse;
use crate::scope::CausalEdge;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use uuid::Uuid;

/// Causal walker.
#[derive(Debug, Clone)]
pub struct CausalWalker {
    edges: Arc<CausalEdgeRepo>,
}

impl CausalWalker {
    /// Construct.
    #[must_use]
    pub fn new(edges: Arc<CausalEdgeRepo>) -> Self {
        Self { edges }
    }

    /// BFS up to `depth` levels.
    pub async fn walk(&self, subject: Uuid, depth: u32) -> common::Result<CausalTraceResponse> {
        let descendants = self.bfs(subject, depth, Direction::Forward).await?;
        let ancestors = self.bfs(subject, depth, Direction::Backward).await?;
        Ok(CausalTraceResponse {
            subject,
            ancestors,
            descendants,
            depth,
        })
    }

    async fn bfs(
        &self,
        start: Uuid,
        depth: u32,
        direction: Direction,
    ) -> common::Result<Vec<CausalEdge>> {
        let mut visited: HashSet<Uuid> = HashSet::new();
        visited.insert(start);
        let mut frontier: VecDeque<(Uuid, u32)> = VecDeque::new();
        frontier.push_back((start, 0));
        let mut out = Vec::new();
        while let Some((node, d)) = frontier.pop_front() {
            if d >= depth {
                continue;
            }
            let edges = match direction {
                Direction::Forward => self.edges.by_from(node).await?,
                Direction::Backward => self.edges.by_to(node).await?,
            };
            for edge in edges {
                let next = match direction {
                    Direction::Forward => edge.to_id,
                    Direction::Backward => edge.from_id,
                };
                if visited.insert(next) {
                    frontier.push_back((next, d + 1));
                    out.push(edge);
                }
            }
        }
        Ok(out)
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Forward,
    Backward,
}
