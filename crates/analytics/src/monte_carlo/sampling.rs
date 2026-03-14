//! Seeded RNG helpers and Cholesky decomposition for correlated returns.

use common::Decimal;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Create a deterministic RNG from a base seed and run index.
pub fn create_rng(base_seed: u64, run_index: u32) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(base_seed.wrapping_add(run_index as u64))
}

/// Cholesky decomposition of a symmetric positive-definite matrix.
/// Returns lower triangular matrix L such that A = L * L^T.
/// Returns None if matrix is not positive definite.
pub fn cholesky_decompose(matrix: &[Vec<Decimal>]) -> Option<Vec<Vec<Decimal>>> {
    let n = matrix.len();
    let mut l = vec![vec![Decimal::ZERO; n]; n];

    for i in 0..n {
        for j in 0..=i {
            let mut sum = Decimal::ZERO;
            for (li_k, lj_k) in l[i].iter().zip(l[j].iter()).take(j) {
                sum += li_k * lj_k;
            }

            if i == j {
                let diag = matrix[i][i] - sum;
                if diag <= Decimal::ZERO {
                    return None;
                }
                l[i][j] = decimal_sqrt(diag)?;
            } else {
                if l[j][j] == Decimal::ZERO {
                    return None;
                }
                l[i][j] = (matrix[i][j] - sum) / l[j][j];
            }
        }
    }

    Some(l)
}

/// Approximate square root of a Decimal using Newton's method.
/// Returns None for negative inputs.
pub(crate) fn decimal_sqrt(val: Decimal) -> Option<Decimal> {
    if val < Decimal::ZERO {
        return None;
    }
    if val == Decimal::ZERO {
        return Some(Decimal::ZERO);
    }

    let two = Decimal::new(2, 0);
    let epsilon = Decimal::new(1, 12);
    let mut guess = val / two;

    for _ in 0..100 {
        let next = (guess + val / guess) / two;
        let diff = if next > guess {
            next - guess
        } else {
            guess - next
        };
        if diff < epsilon {
            return Some(next);
        }
        guess = next;
    }

    Some(guess)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn cholesky_2x2_identity() {
        let m = vec![vec![dec!(1), dec!(0)], vec![dec!(0), dec!(1)]];
        let l = cholesky_decompose(&m).unwrap();
        assert_eq!(l[0][0], dec!(1));
        assert_eq!(l[1][1], dec!(1));
        assert_eq!(l[1][0], dec!(0));
    }

    #[test]
    fn cholesky_not_positive_definite_returns_none() {
        let m = vec![vec![dec!(1), dec!(2)], vec![dec!(2), dec!(1)]];
        assert!(cholesky_decompose(&m).is_none());
    }

    #[test]
    fn decimal_sqrt_basic() {
        let result = decimal_sqrt(dec!(4)).unwrap();
        let diff = (result - dec!(2)).abs();
        assert!(diff < dec!(0.0001));
    }

    #[test]
    fn decimal_sqrt_negative_returns_none() {
        assert!(decimal_sqrt(dec!(-1)).is_none());
    }

    #[test]
    fn rng_deterministic() {
        let mut rng1 = create_rng(42, 0);
        let mut rng2 = create_rng(42, 0);
        let v1: u64 = rand::Rng::random(&mut rng1);
        let v2: u64 = rand::Rng::random(&mut rng2);
        assert_eq!(v1, v2);
    }
}
