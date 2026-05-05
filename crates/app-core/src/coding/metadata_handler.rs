use crate::AppCore;
use common::Result;

/// Result for coding_thread_metadata_generate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataGenerateResult {
    pub title: String,
    pub worktree_name: String,
}

impl AppCore {
    /// Generate a short title and worktree name from a user prompt.
    ///
    /// Uses a simple heuristic for now; can be upgraded to LLM-backed generation
    /// via the cheapest available provider.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_thread_metadata_generate(
        &self,
        workspace_id: &str,
        prompt: &str,
    ) -> Result<MetadataGenerateResult> {
        let _ = workspace_id; // Future: load workspace context for better titles

        // Simple heuristic: take first 4 significant words, lowercase, kebab-case
        let title = generate_title(prompt);
        let worktree_name = slugify(&title);

        tracing::info!(
            prompt_len = prompt.len(),
            title = %title,
            worktree_name = %worktree_name,
            "Metadata generated"
        );

        Ok(MetadataGenerateResult {
            title,
            worktree_name,
        })
    }
}

fn generate_title(prompt: &str) -> String {
    let stop_words: &[&str] = &[
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "can", "shall",
        "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "into", "about", "like",
        "through", "after", "over", "between", "out", "against", "during", "without", "before",
        "under", "around", "among", "and", "but", "or", "nor", "not", "so", "yet", "both",
        "either", "neither", "each", "every", "all", "any", "few", "more", "most", "other", "some",
        "such", "no", "only", "own", "same", "than", "too", "very", "just", "because", "if",
        "when", "where", "how", "what", "which", "who", "whom", "this", "that", "these", "those",
        "i", "me", "my", "we", "our", "you", "your", "he", "him", "his", "she", "her", "it", "its",
        "they", "them", "their", "please", "help", "want", "need", "make", "create", "add", "fix",
        "update", "write",
    ];

    let words: Vec<&str> = prompt
        .split_whitespace()
        .filter(|w| {
            let lower = w.to_lowercase();
            !stop_words.contains(&lower.as_str()) && lower.len() > 1
        })
        .take(4)
        .collect();

    if words.is_empty() {
        // Fallback: take first 4 words regardless
        prompt
            .split_whitespace()
            .take(4)
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        words.join(" ")
    }
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_title_extracts_keywords() {
        let title = generate_title("Please help me fix the authentication bug in the login flow");
        assert!(title.contains("authentication"));
        assert!(title.contains("bug"));
        assert!(title.contains("login"));
        assert!(title.contains("flow"));
    }

    #[test]
    fn slugify_produces_kebab_case() {
        assert_eq!(slugify("Fix Auth Bug"), "fix-auth-bug");
        assert_eq!(slugify("  hello   world  "), "hello-world");
    }

    #[test]
    fn generate_title_fallback_on_stop_words_only() {
        let title = generate_title("please help me");
        assert!(!title.is_empty());
    }
}
