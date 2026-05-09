//! Heuristic classifier for TodoItem titles that look like verification steps
//! (Run / Test / Check / Verify / Build).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationVerb {
    Run,
    Test,
    Check,
    Verify,
    Build,
}

impl VerificationVerb {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Run    => "Run",
            Self::Test   => "Test",
            Self::Check  => "Check",
            Self::Verify => "Verify",
            Self::Build  => "Build",
        }
    }
}

pub fn classify(title: &str) -> Option<VerificationVerb> {
    let trimmed = title.trim();
    if trimmed.len() < 3 {
        return None;
    }
    let first_token = trimmed.split_whitespace().next()?;
    let lower = first_token.to_ascii_lowercase();
    let cleaned = lower.trim_end_matches(|c: char| !c.is_alphanumeric());
    match cleaned {
        "run" | "running"                       => Some(VerificationVerb::Run),
        "test" | "tests"                        => Some(VerificationVerb::Test),
        "check" | "checking"                    => Some(VerificationVerb::Check),
        "verify" | "verifies" | "verifying"     => Some(VerificationVerb::Verify),
        "build" | "rebuild" | "compile"         => Some(VerificationVerb::Build),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_each_verb() {
        assert_eq!(classify("Run integration tests"), Some(VerificationVerb::Run));
        assert_eq!(classify("Test the supervisor"),   Some(VerificationVerb::Test));
        assert_eq!(classify("Check migrations"),      Some(VerificationVerb::Check));
        assert_eq!(classify("Verify migration safety"), Some(VerificationVerb::Verify));
        assert_eq!(classify("Build release binary"),  Some(VerificationVerb::Build));
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(classify("RUN tests"), Some(VerificationVerb::Run));
        assert_eq!(classify("test x"),    Some(VerificationVerb::Test));
    }

    #[test]
    fn handles_conjugations() {
        assert_eq!(classify("Running the suite"),  Some(VerificationVerb::Run));
        assert_eq!(classify("Verifying constraints"), Some(VerificationVerb::Verify));
        assert_eq!(classify("Rebuild the docs"),   Some(VerificationVerb::Build));
    }

    #[test]
    fn trailing_punctuation_ok() {
        assert_eq!(classify("verify."), Some(VerificationVerb::Verify));
        assert_eq!(classify("Test:"),   Some(VerificationVerb::Test));
    }

    #[test]
    fn rejects_unrelated_titles() {
        assert_eq!(classify("Refactor the supervisor"), None);
        assert_eq!(classify("Add a new tool"),          None);
        assert_eq!(classify("Fix bug in injector"),     None);
    }

    #[test]
    fn rejects_too_short() {
        assert_eq!(classify(""),    None);
        assert_eq!(classify("a"),   None);
        assert_eq!(classify("ab"),  None);
        assert_eq!(classify("Run"), Some(VerificationVerb::Run));    // 3 chars OK
    }
}
