/// Tier 1 — Detailed Summary prompt.
///
/// Preserves decisions, code references, action items, reasoning.
/// Target: ~35% of original length.
pub const TIER1_INSTRUCTIONS: &str = "\
Summarize each conversation turn below. For each turn, preserve:
- Decisions made and their reasoning
- Action items or commitments
- File paths, function names, IDs, or other specific references
- Key questions asked and answers given
- Errors encountered and how they were resolved
- Any constraints or requirements stated

Preserve temporal order of events. Use bullet points. Never invent information.
Keep technical details (exact names, numbers, paths). Remove pleasantries, \
repetition, and verbose explanations. Target ~35% of original length.";

/// Tier 2 — Condensed Gist prompt.
///
/// Outcomes only, maximum compression.
/// Target: ~12% of original length.
pub const TIER2_INSTRUCTIONS: &str = "\
For each conversation turn below, extract ONLY:
- The final outcome or decision (one sentence)
- Any unresolved item that affects later conversation (prefix with \"UNRESOLVED:\")

No code, no file paths, no reasoning chains. Maximum 2 sentences per turn. \
Target ~12% of original length.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier1_mentions_decisions() {
        assert!(TIER1_INSTRUCTIONS.contains("Decisions made"));
        assert!(TIER1_INSTRUCTIONS.contains("File paths"));
        assert!(TIER1_INSTRUCTIONS.contains("35%"));
    }

    #[test]
    fn tier2_mentions_outcomes_only() {
        assert!(TIER2_INSTRUCTIONS.contains("ONLY"));
        assert!(TIER2_INSTRUCTIONS.contains("UNRESOLVED"));
        assert!(TIER2_INSTRUCTIONS.contains("12%"));
        assert!(TIER2_INSTRUCTIONS.contains("No code"));
    }
}
