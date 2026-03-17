/// Detect the gradient drop point in a descending-sorted score list.
/// Returns Some(i) where the sharp drop begins, or None if no gradient found.
pub fn detect_gradient(scores: &[f64], g: f64) -> Option<usize> {
    if scores.len() <= 1 {
        return None;
    }
    let mut prev = scores[0];
    for (i, &score) in scores.iter().enumerate().skip(1) {
        if score < prev * g {
            return Some(i);
        }
        prev = score;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_scores() {
        assert_eq!(detect_gradient(&[], 0.6), None);
    }

    #[test]
    fn single_score() {
        assert_eq!(detect_gradient(&[0.9], 0.6), None);
    }

    #[test]
    fn uniform_low_scores_no_gradient() {
        assert_eq!(detect_gradient(&[0.3, 0.28, 0.26, 0.25], 0.6), None);
    }

    #[test]
    fn clear_single_match() {
        assert_eq!(detect_gradient(&[0.95, 0.3, 0.28, 0.25], 0.6), Some(1));
    }

    #[test]
    fn multiple_matches_then_drop() {
        assert_eq!(detect_gradient(&[0.95, 0.90, 0.3, 0.28], 0.6), Some(2));
    }

    #[test]
    fn gradual_decline_no_gradient() {
        assert_eq!(detect_gradient(&[0.9, 0.85, 0.80, 0.76], 0.6), None);
    }
}
