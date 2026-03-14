use chrono::NaiveDate;
use common::Decimal;
use rust_decimal::prelude::*;
use serde::Serialize;

use crate::InvestmentCashFlow;

/// Result of returns analysis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnsResult {
    /// Time-Weighted Return.
    pub twr: Decimal,
    /// Money-Weighted Return (may fail to converge).
    pub mwr: Option<Decimal>,
    /// Annualized TWR (CAGR).
    pub twr_annualized: Decimal,
    pub total_gain: Decimal,
    pub total_invested: Decimal,
}

impl super::PortfolioAnalyzer {
    /// Calculate portfolio returns using TWR (Modified Dietz) and MWR (IRR).
    pub fn returns(
        start_value: Decimal,
        end_value: Decimal,
        cash_flows: &[InvestmentCashFlow],
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> ReturnsResult {
        let twr =
            Self::modified_dietz_twr(start_value, end_value, cash_flows, start_date, end_date);
        let mwr = Self::newton_irr(start_value, end_value, cash_flows, start_date, end_date);

        // Annualize TWR.
        let days = (end_date - start_date).num_days() as f64;
        let years = days / 365.25;
        let twr_annualized = if years > 0.0 && twr > Decimal::new(-1, 0) {
            let one = Decimal::ONE;
            let total = one + twr;
            let years_dec = Decimal::from_f64_retain(years).unwrap_or(Decimal::ONE);
            if years_dec > Decimal::ZERO {
                let ln_total = total.ln();
                let annual_ln = ln_total / years_dec;
                annual_ln.exp() - one
            } else {
                twr
            }
        } else {
            twr
        };

        let total_invested: Decimal = cash_flows
            .iter()
            .filter(|cf| cf.amount > Decimal::ZERO)
            .map(|cf| cf.amount)
            .sum();
        let total_gain = end_value - start_value - total_invested;

        ReturnsResult {
            twr,
            mwr,
            twr_annualized,
            total_gain,
            total_invested,
        }
    }

    /// Modified Dietz TWR.
    fn modified_dietz_twr(
        start_value: Decimal,
        end_value: Decimal,
        cash_flows: &[InvestmentCashFlow],
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Decimal {
        let total_days = (end_date - start_date).num_days();
        if total_days <= 0 {
            return Decimal::ZERO;
        }

        let mut weighted_cf = Decimal::ZERO;
        let mut total_cf = Decimal::ZERO;

        for cf in cash_flows {
            let days_remaining = (end_date - cf.date).num_days();
            let weight = Decimal::from_f64_retain(days_remaining as f64 / total_days as f64)
                .unwrap_or(Decimal::ZERO);
            weighted_cf += cf.amount * weight;
            total_cf += cf.amount;
        }

        let denominator = start_value + weighted_cf;
        if denominator == Decimal::ZERO {
            return Decimal::ZERO;
        }

        (end_value - start_value - total_cf) / denominator
    }

    /// Newton's method IRR for MWR.
    fn newton_irr(
        start_value: Decimal,
        end_value: Decimal,
        cash_flows: &[InvestmentCashFlow],
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Option<Decimal> {
        // Solve: -start_value + sum(cf_i / (1+r)^t_i) + end_value / (1+r)^T = 0
        let total_days = (end_date - start_date).num_days() as f64;
        if total_days <= 0.0 {
            return None;
        }

        let mut r = 0.1_f64; // initial guess

        for _ in 0..100 {
            let mut npv = -start_value.to_f64().unwrap_or(0.0);
            let mut dnpv = 0.0_f64;

            for cf in cash_flows {
                let t = (cf.date - start_date).num_days() as f64 / 365.25;
                let disc = (1.0 + r).powf(t);
                let amount = cf.amount.to_f64().unwrap_or(0.0);
                npv += amount / disc;
                dnpv -= t * amount / (disc * (1.0 + r));
            }

            let t_end = total_days / 365.25;
            let disc_end = (1.0 + r).powf(t_end);
            let end_val = end_value.to_f64().unwrap_or(0.0);
            npv += end_val / disc_end;
            dnpv -= t_end * end_val / (disc_end * (1.0 + r));

            if dnpv.abs() < 1e-12 {
                return None;
            }

            let new_r = r - npv / dnpv;
            if (new_r - r).abs() < 1e-10 {
                return Decimal::from_f64_retain(new_r);
            }
            r = new_r;

            // Bounds check.
            if !(-0.99..=10.0).contains(&r) {
                return None;
            }
        }

        None // Did not converge
    }
}
