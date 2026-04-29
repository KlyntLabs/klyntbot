use kca_e2e::replayer::ReplayMeasurements;

#[derive(Debug, Default, Clone)]
pub struct LatencyDashboard {
    pub hot_path_p50_ms: u64,
    pub hot_path_p95_ms: u64,
    pub warm_path_p50_ms: u64,
    pub warm_path_p95_ms: u64,
    pub cold_path_minutes: f64,
    pub retrieval_p95_ms: u64,
}

pub fn compute(m: &ReplayMeasurements) -> LatencyDashboard {
    let mut s = m.turn_latencies_ms.clone();
    s.sort();
    let p50 = if s.is_empty() { 0 } else { s[s.len() / 2] };
    let p95 = if s.is_empty() {
        0
    } else {
        s[(s.len() as f64 * 0.95) as usize]
    };
    LatencyDashboard {
        hot_path_p50_ms: p50,
        hot_path_p95_ms: p95,
        ..Default::default()
    }
}
