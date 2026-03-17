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
}
