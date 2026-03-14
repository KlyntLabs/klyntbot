//! Return distribution models for Monte Carlo simulation.

use common::Decimal;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// Draw a return from a log-normal distribution.
/// mean_return and std_dev are in simple return space (e.g., 0.07 for 7%).
pub fn draw_log_normal(rng: &mut ChaCha8Rng, mean_return: Decimal, std_dev: Decimal) -> Decimal {
    let u1: f64 = rng.random::<f64>().max(1e-10);
    let u2: f64 = rng.random::<f64>();

    let z = ((-2.0 * u1.ln()).sqrt()) * (2.0 * std::f64::consts::PI * u2).cos();

    let z_dec = Decimal::from_f64_retain(z).unwrap_or(Decimal::ZERO);
    let simple_return = mean_return + std_dev * z_dec;

    let floor = Decimal::new(-999, 3); // -0.999
    if simple_return < floor {
        floor
    } else {
        simple_return
    }
}

/// Draw a return by sampling from historical data with replacement.
pub fn draw_bootstrap(rng: &mut ChaCha8Rng, historical_returns: &[Decimal]) -> Decimal {
    if historical_returns.is_empty() {
        return Decimal::ZERO;
    }
    let idx = rng.random_range(0..historical_returns.len());
    historical_returns[idx]
}

/// Draw correlated returns for multiple asset classes.
pub fn draw_correlated_returns(
    rng: &mut ChaCha8Rng,
    means: &[Decimal],
    std_devs: &[Decimal],
    cholesky_l: &[Vec<Decimal>],
) -> Vec<Decimal> {
    let n = means.len();

    let mut z = Vec::with_capacity(n);
    for _ in 0..n {
        let u1: f64 = rng.random::<f64>().max(1e-10);
        let u2: f64 = rng.random::<f64>();
        let normal = ((-2.0 * u1.ln()).sqrt()) * (2.0 * std::f64::consts::PI * u2).cos();
        z.push(Decimal::from_f64_retain(normal).unwrap_or(Decimal::ZERO));
    }

    let mut correlated = vec![Decimal::ZERO; n];
    for i in 0..n {
        let mut sum = Decimal::ZERO;
        for j in 0..=i {
            sum += cholesky_l[i][j] * z[j];
        }
        let ret = means[i] + std_devs[i] * sum;
        let floor = Decimal::new(-999, 3);
        correlated[i] = if ret < floor { floor } else { ret };
    }

    correlated
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use rust_decimal_macros::dec;

    #[test]
    fn log_normal_deterministic_with_seed() {
        let mut rng1 = ChaCha8Rng::seed_from_u64(42);
        let mut rng2 = ChaCha8Rng::seed_from_u64(42);
        let r1 = draw_log_normal(&mut rng1, dec!(0.07), dec!(0.15));
        let r2 = draw_log_normal(&mut rng2, dec!(0.07), dec!(0.15));
        assert_eq!(r1, r2);
    }

    #[test]
    fn bootstrap_samples_from_data() {
        let data = vec![dec!(0.10), dec!(-0.05), dec!(0.15)];
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let r = draw_bootstrap(&mut rng, &data);
        assert!(data.contains(&r));
    }

    #[test]
    fn bootstrap_empty_returns_zero() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        assert_eq!(draw_bootstrap(&mut rng, &[]), Decimal::ZERO);
    }
}
