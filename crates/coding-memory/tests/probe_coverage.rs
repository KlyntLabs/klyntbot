use coding_memory::recall::probe::{ProbeVerdict, RetrievalQualityProbe};

#[test]
fn empty_results_score_zero() {
    let p = RetrievalQualityProbe::new(0.3);
    assert_eq!(p.score(&[]), 0.0);
    assert_eq!(p.verdict(&[]), ProbeVerdict::Escalate);
}

#[test]
fn coverage_is_mean_minus_min() {
    let p = RetrievalQualityProbe::new(0.3);
    let s = p.score(&[0.9, 0.8, 0.5]);
    let expected = ((0.9 + 0.8 + 0.5) / 3.0) - 0.5;
    assert!((s - expected).abs() < 1e-5, "got {s}, want {expected}");
}

#[test]
fn high_coverage_passes() {
    let p = RetrievalQualityProbe::new(0.3);
    assert_eq!(p.verdict(&[0.95, 0.6, 0.3]), ProbeVerdict::Sufficient);
}

#[test]
fn low_coverage_escalates() {
    let p = RetrievalQualityProbe::new(0.3);
    assert_eq!(p.verdict(&[0.4, 0.2, 0.05]), ProbeVerdict::Escalate);
}
