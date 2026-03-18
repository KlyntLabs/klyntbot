#[derive(Debug, Clone, PartialEq)]
pub enum QueryCategory {
    SingleHop,
    MultiHop,
    GlobalAggregation,
    PassThrough,
}

pub fn classify_heuristic(query: &str) -> QueryCategory {
    let q = query.to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();

    if words.len() <= 2 {
        return QueryCategory::PassThrough;
    }

    let global_keywords = [
        "how many",
        "count",
        "total",
        "list all",
        "sum of",
        "across all",
    ];
    if global_keywords.iter().any(|kw| q.contains(kw)) {
        return QueryCategory::GlobalAggregation;
    }

    let multi_hop_keywords = [
        "compare", "differ", "relate", "between", "how does", "affect", "versus", "vs",
    ];
    if multi_hop_keywords.iter().any(|kw| q.contains(kw)) {
        return QueryCategory::MultiHop;
    }

    QueryCategory::SingleHop
}

/// Classify with heuristic first, then LLM fallback for ambiguous long queries.
pub async fn classify_with_llm_fallback(
    query: &str,
    llm: &dyn crate::operators::OperatorLlm,
) -> QueryCategory {
    let heuristic_result = classify_heuristic(query);

    // Only use LLM fallback for ambiguous cases:
    // - Heuristic returned SingleHop (default/fallback)
    // - Query is long enough to be potentially complex (6+ words)
    let words = query.split_whitespace().count();
    if heuristic_result != QueryCategory::SingleHop || words < 6 {
        return heuristic_result;
    }

    let prompt = format!(
        "Classify this query into exactly one category:\n\
         - SingleHop: simple lookup, one entity, direct answer\n\
         - MultiHop: requires connecting info across multiple topics\n\
         - GlobalAggregation: counting, listing all, summarizing across everything\n\
         - PassThrough: greeting, chitchat, not a knowledge question\n\n\
         Reply with ONLY the category name.\n\n\
         Query: \"{}\"",
        query
    );

    match llm
        .complete(
            "You classify queries. Reply with one word: SingleHop, MultiHop, GlobalAggregation, or PassThrough.",
            &prompt,
        )
        .await
    {
        Ok(response) => {
            let trimmed = response.trim();
            match trimmed {
                "MultiHop" => QueryCategory::MultiHop,
                "GlobalAggregation" => QueryCategory::GlobalAggregation,
                "PassThrough" => QueryCategory::PassThrough,
                _ => QueryCategory::SingleHop,
            }
        }
        Err(_) => heuristic_result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_single_hop() {
        assert_eq!(
            classify_heuristic("What is the deadline for Project Alpha?"),
            QueryCategory::SingleHop
        );
    }

    #[test]
    fn classify_multi_hop() {
        assert_eq!(
            classify_heuristic("How does my finance goal relate to work projects?"),
            QueryCategory::MultiHop
        );
    }

    #[test]
    fn classify_global() {
        assert_eq!(
            classify_heuristic("How many tasks are overdue across all projects?"),
            QueryCategory::GlobalAggregation
        );
    }

    #[test]
    fn classify_passthrough() {
        assert_eq!(classify_heuristic("hello"), QueryCategory::PassThrough);
        assert_eq!(classify_heuristic("hi there"), QueryCategory::PassThrough);
    }

    #[tokio::test]
    async fn llm_fallback_refines_ambiguous() {
        use async_trait::async_trait;

        struct MockLlm;
        #[async_trait]
        impl crate::operators::OperatorLlm for MockLlm {
            async fn complete(&self, _system: &str, _prompt: &str) -> common::Result<String> {
                Ok("MultiHop".to_string())
            }
        }

        let result = classify_with_llm_fallback(
            "What is the connection between my finance goals and the project timeline",
            &MockLlm,
        )
        .await;
        assert_eq!(result, QueryCategory::MultiHop);
    }

    #[tokio::test]
    async fn llm_fallback_skips_short_queries() {
        use async_trait::async_trait;

        struct PanicLlm;
        #[async_trait]
        impl crate::operators::OperatorLlm for PanicLlm {
            async fn complete(&self, _system: &str, _prompt: &str) -> common::Result<String> {
                panic!("LLM should not be called for short queries");
            }
        }

        // Short query → PassThrough, no LLM call
        let result = classify_with_llm_fallback("hello there", &PanicLlm).await;
        assert_eq!(result, QueryCategory::PassThrough);
    }

    #[tokio::test]
    async fn llm_fallback_skips_clear_heuristic() {
        use async_trait::async_trait;

        struct PanicLlm;
        #[async_trait]
        impl crate::operators::OperatorLlm for PanicLlm {
            async fn complete(&self, _system: &str, _prompt: &str) -> common::Result<String> {
                panic!("LLM should not be called for clear heuristic matches");
            }
        }

        // Clear MultiHop keyword → no LLM call
        let result = classify_with_llm_fallback(
            "How does my finance goal relate to work projects and deadlines?",
            &PanicLlm,
        )
        .await;
        assert_eq!(result, QueryCategory::MultiHop);
    }
}
