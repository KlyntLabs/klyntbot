//! FIRE calculator variants: Traditional, Coast, Lean, Fat.

use common::Decimal;
use rust_decimal::prelude::*;
use serde::Serialize;

/// Parameters for Traditional FIRE calculation.
#[derive(Debug, Clone)]
pub struct FIREParams {
    pub annual_expenses: Decimal,
    pub current_portfolio: Decimal,
    pub monthly_savings: Decimal,
    pub expected_return: Decimal,       // nominal (e.g., 0.07)
    pub inflation_rate: Decimal,        // e.g., 0.03
    pub withdrawal_rates: Vec<Decimal>, // e.g., [0.04, 0.035, 0.03]
}

/// A FIRE number for a specific withdrawal rate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FIRENumber {
    pub withdrawal_rate: Decimal,
    pub fire_number: Decimal,
}

/// Result of a Traditional FIRE calculation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FIREResult {
    pub fire_numbers: Vec<FIRENumber>,
    pub months_to_fire: Option<u32>, // None = unreachable
    pub years_to_fire: Option<Decimal>,
    pub current_progress: Decimal, // current / fire_number (0.0 to 1.0+)
    pub real_return: Decimal,
}

/// Parameters for Coast FIRE.
#[derive(Debug, Clone)]
pub struct CoastFIREParams {
    pub current_portfolio: Decimal,
    pub current_age: u32,
    pub target_age: u32,
    pub annual_expenses_at_retirement: Decimal,
    pub expected_return: Decimal,
    pub inflation_rate: Decimal,
    pub withdrawal_rate: Decimal,
}

/// Result of a Coast FIRE calculation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoastFIREResult {
    pub coast_number: Decimal,
    pub fire_number: Decimal,
    pub is_coast_fire: bool,
    pub surplus_or_deficit: Decimal,
    pub years_to_coast: Option<u32>,
}

/// Parameters for Lean FIRE.
#[derive(Debug, Clone)]
pub struct LeanFIREParams {
    pub essential_expenses: Decimal,
    pub current_portfolio: Decimal,
    pub monthly_savings: Decimal,
    pub expected_return: Decimal,
    pub inflation_rate: Decimal,
    pub withdrawal_rate: Decimal,
}

/// Parameters for Fat FIRE.
#[derive(Debug, Clone)]
pub struct FatFIREParams {
    pub desired_annual_spending: Decimal,
    pub current_portfolio: Decimal,
    pub monthly_savings: Decimal,
    pub expected_return: Decimal,
    pub inflation_rate: Decimal,
    pub withdrawal_rate: Decimal,
}

pub struct FIRECalculator;

impl FIRECalculator {
    /// Traditional FIRE: calculate FIRE number and time to reach it.
    pub fn traditional(params: &FIREParams) -> FIREResult {
        let fire_numbers: Vec<FIRENumber> = params
            .withdrawal_rates
            .iter()
            .filter(|rate| **rate > Decimal::ZERO)
            .map(|rate| FIRENumber {
                withdrawal_rate: *rate,
                fire_number: params.annual_expenses / *rate,
            })
            .collect();

        if fire_numbers.is_empty() {
            return FIREResult {
                fire_numbers: Vec::new(),
                months_to_fire: None,
                years_to_fire: None,
                current_progress: Decimal::ZERO,
                real_return: Decimal::ZERO,
            };
        }

        // Use first (primary) withdrawal rate for time calculations
        let primary_fire = fire_numbers[0].fire_number;
        let current_progress = if primary_fire > Decimal::ZERO {
            params.current_portfolio / primary_fire
        } else {
            Decimal::ONE
        };

        // Real return = (1 + nominal) / (1 + inflation) - 1
        let one = Decimal::ONE;
        let real_return = (one + params.expected_return) / (one + params.inflation_rate) - one;

        let months_to_fire = if params.current_portfolio >= primary_fire {
            Some(0u32)
        } else if real_return <= Decimal::ZERO && params.monthly_savings <= Decimal::ZERO {
            None // Unreachable
        } else {
            Self::compute_months_to_target(
                params.current_portfolio,
                primary_fire,
                params.monthly_savings,
                real_return,
            )
        };

        let years_to_fire = months_to_fire.map(|m| Decimal::new(m as i64, 0) / Decimal::new(12, 0));

        FIREResult {
            fire_numbers,
            months_to_fire,
            years_to_fire,
            current_progress,
            real_return,
        }
    }

    /// Coast FIRE: how much you need now so compound growth reaches FIRE number.
    pub fn coast(params: &CoastFIREParams) -> CoastFIREResult {
        let fire_number = params.annual_expenses_at_retirement / params.withdrawal_rate;
        let one = Decimal::ONE;
        let real_return = (one + params.expected_return) / (one + params.inflation_rate) - one;

        if real_return <= Decimal::ZERO {
            // With negative real returns, coast doesn't work
            return CoastFIREResult {
                coast_number: fire_number, // Would need the full amount now
                fire_number,
                is_coast_fire: false,
                surplus_or_deficit: params.current_portfolio - fire_number,
                years_to_coast: None,
            };
        }

        let years = params.target_age.saturating_sub(params.current_age);
        // coast_number = fire_number / (1 + real_return)^years
        let growth_factor = (one + real_return).powu(years as u64);
        let coast_number = fire_number / growth_factor;

        let is_coast_fire = params.current_portfolio >= coast_number;
        let surplus_or_deficit = params.current_portfolio - coast_number;

        // If not yet at coast, how many more years until current portfolio grows to coast_number?
        // This only makes sense if we have positive real returns
        let years_to_coast = if is_coast_fire { Some(0) } else { None };

        CoastFIREResult {
            coast_number,
            fire_number,
            is_coast_fire,
            surplus_or_deficit,
            years_to_coast,
        }
    }

    /// Lean FIRE: uses essential expenses only.
    pub fn lean(params: &LeanFIREParams) -> FIREResult {
        Self::traditional(&FIREParams {
            annual_expenses: params.essential_expenses,
            current_portfolio: params.current_portfolio,
            monthly_savings: params.monthly_savings,
            expected_return: params.expected_return,
            inflation_rate: params.inflation_rate,
            withdrawal_rates: vec![params.withdrawal_rate],
        })
    }

    /// Fat FIRE: uses desired lifestyle spending.
    pub fn fat(params: &FatFIREParams) -> FIREResult {
        Self::traditional(&FIREParams {
            annual_expenses: params.desired_annual_spending,
            current_portfolio: params.current_portfolio,
            monthly_savings: params.monthly_savings,
            expected_return: params.expected_return,
            inflation_rate: params.inflation_rate,
            withdrawal_rates: vec![params.withdrawal_rate],
        })
    }

    /// Compute months to reach a target value given current value, monthly payment,
    /// and annual real return.
    ///
    /// Uses: months = ceil(ln((FV * r + pmt) / (PV * r + pmt)) / ln(1 + r))
    /// where r is monthly real rate and pmt is monthly savings.
    fn compute_months_to_target(
        current: Decimal,
        target: Decimal,
        monthly_payment: Decimal,
        annual_real_return: Decimal,
    ) -> Option<u32> {
        let one = Decimal::ONE;
        let twelve = Decimal::new(12, 0);

        if current >= target {
            return Some(0);
        }

        if monthly_payment <= Decimal::ZERO && annual_real_return <= Decimal::ZERO {
            return None;
        }

        // Monthly rate = (1 + annual_real)^(1/12) - 1
        // Approximate using: monthly ~ annual / 12 (for small rates)
        // More accurate: use ln/exp
        let monthly_rate = if annual_real_return > Decimal::ZERO {
            // (1 + r)^(1/12) - 1
            let ln_1_plus_r = (one + annual_real_return).ln();
            let monthly_ln = ln_1_plus_r / twelve;
            monthly_ln.exp() - one
        } else {
            Decimal::ZERO
        };

        if monthly_rate == Decimal::ZERO {
            // Simple: months = (target - current) / monthly_payment
            if monthly_payment <= Decimal::ZERO {
                return None;
            }
            let months = (target - current) / monthly_payment;
            return Some(months.ceil().to_u32().unwrap_or(u32::MAX));
        }

        // FV formula: FV = PV*(1+r)^n + PMT*((1+r)^n - 1)/r
        // Solve for n: n = ln((FV*r + PMT) / (PV*r + PMT)) / ln(1+r)
        let numerator = target * monthly_rate + monthly_payment;
        let denominator = current * monthly_rate + monthly_payment;

        if denominator <= Decimal::ZERO || numerator <= Decimal::ZERO {
            return None;
        }

        let ratio = numerator / denominator;
        if ratio <= Decimal::ZERO {
            return None;
        }

        let ln_ratio = ratio.ln();
        let ln_1_plus_r = (one + monthly_rate).ln();

        if ln_1_plus_r <= Decimal::ZERO {
            return None;
        }

        let months = ln_ratio / ln_1_plus_r;
        let months_ceil = months.ceil();

        months_ceil.to_u32()
    }
}
