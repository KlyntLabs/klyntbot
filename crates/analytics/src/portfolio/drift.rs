use std::collections::HashMap;

use common::Decimal;
use serde::Serialize;

use crate::{AllocationTarget, Holding};

/// Result of drift analysis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftResult {
    pub allocations: Vec<AssetAllocation>,
    pub needs_rebalancing: bool,
    /// 0.0 = perfect, higher = more drift (sum of |drift| across all classes).
    pub drift_score: Decimal,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetAllocation {
    pub asset_class: String,
    pub current_weight: Decimal,
    pub target_weight: Decimal,
    /// current_weight - target_weight (signed).
    pub drift: Decimal,
    /// |drift| / target_weight.
    pub drift_pct: Decimal,
    pub current_value: Decimal,
    pub target_value: Decimal,
    pub exceeds_band: bool,
}

/// Rebalancing strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebalanceStrategy {
    /// Buy and sell to match targets.
    FullRebalance,
    /// Only buy (using new money).
    ContributionOnly,
    /// Only rebalance asset classes exceeding tolerance.
    ThresholdOnly,
}

/// A rebalancing suggestion.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebalanceSuggestion {
    pub asset_class: String,
    /// "buy" or "sell".
    pub action: String,
    pub amount: Decimal,
    pub from_weight: Decimal,
    pub to_weight: Decimal,
}

/// Result of rebalancing analysis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebalanceResult {
    pub suggestions: Vec<RebalanceSuggestion>,
    pub total_portfolio_value: Decimal,
    /// Amount of new money needed (for `ContributionOnly`).
    pub contribution_needed: Decimal,
}

pub struct PortfolioAnalyzer;

impl PortfolioAnalyzer {
    /// Compute allocation drift from targets.
    pub fn allocation_drift(holdings: &[Holding], targets: &[AllocationTarget]) -> DriftResult {
        // Sum holdings by asset_class.
        let mut class_values: HashMap<String, Decimal> = HashMap::new();
        for h in holdings {
            *class_values
                .entry(h.asset_class.clone())
                .or_insert(Decimal::ZERO) += h.current_value;
        }

        let total_value: Decimal = class_values.values().copied().sum();

        let mut allocations = Vec::with_capacity(targets.len());
        let mut needs_rebalancing = false;
        let mut drift_score = Decimal::ZERO;

        for target in targets {
            let current_value = class_values
                .get(&target.asset_class)
                .copied()
                .unwrap_or(Decimal::ZERO);

            let current_weight = if total_value > Decimal::ZERO {
                current_value / total_value
            } else {
                Decimal::ZERO
            };

            let target_value = target.target_weight * total_value;
            let drift = current_weight - target.target_weight;

            let drift_pct = if target.target_weight > Decimal::ZERO {
                drift.abs() / target.target_weight
            } else {
                Decimal::ZERO
            };

            let exceeds_band = drift_pct > target.tolerance_band;
            if exceeds_band {
                needs_rebalancing = true;
            }

            drift_score += drift.abs();

            allocations.push(AssetAllocation {
                asset_class: target.asset_class.clone(),
                current_weight,
                target_weight: target.target_weight,
                drift,
                drift_pct,
                current_value,
                target_value,
                exceeds_band,
            });
        }

        DriftResult {
            allocations,
            needs_rebalancing,
            drift_score,
        }
    }

    /// Generate rebalancing suggestions.
    pub fn rebalance_suggestions(
        holdings: &[Holding],
        targets: &[AllocationTarget],
        strategy: RebalanceStrategy,
        contribution: Decimal,
        min_trade_amount: Decimal,
    ) -> RebalanceResult {
        // Compute current allocation per asset class.
        let mut class_values: HashMap<String, Decimal> = HashMap::new();
        for h in holdings {
            *class_values
                .entry(h.asset_class.clone())
                .or_insert(Decimal::ZERO) += h.current_value;
        }

        let current_total: Decimal = class_values.values().copied().sum();

        match strategy {
            RebalanceStrategy::FullRebalance => {
                let new_total = current_total + contribution;
                let mut suggestions = Vec::new();

                for target in targets {
                    let current_value = class_values
                        .get(&target.asset_class)
                        .copied()
                        .unwrap_or(Decimal::ZERO);
                    let target_value = target.target_weight * new_total;
                    let diff = target_value - current_value;

                    let from_weight = if current_total > Decimal::ZERO {
                        current_value / current_total
                    } else {
                        Decimal::ZERO
                    };

                    if diff.abs() >= min_trade_amount {
                        let (action, amount) = if diff > Decimal::ZERO {
                            ("buy".to_string(), diff)
                        } else {
                            ("sell".to_string(), diff.abs())
                        };
                        suggestions.push(RebalanceSuggestion {
                            asset_class: target.asset_class.clone(),
                            action,
                            amount,
                            from_weight,
                            to_weight: target.target_weight,
                        });
                    }
                }

                RebalanceResult {
                    suggestions,
                    total_portfolio_value: new_total,
                    contribution_needed: Decimal::ZERO,
                }
            }
            RebalanceStrategy::ContributionOnly => {
                // Distribute the contribution to bring allocations closer to targets.
                // Only buy, never sell.
                let new_total = current_total + contribution;
                let mut suggestions = Vec::new();
                let mut contribution_needed = Decimal::ZERO;

                // Compute the deficit for each underweight class.
                let mut deficits: Vec<(String, Decimal, Decimal, Decimal)> = Vec::new();
                for target in targets {
                    let current_value = class_values
                        .get(&target.asset_class)
                        .copied()
                        .unwrap_or(Decimal::ZERO);
                    let target_value = target.target_weight * new_total;
                    let deficit = target_value - current_value;
                    let from_weight = if current_total > Decimal::ZERO {
                        current_value / current_total
                    } else {
                        Decimal::ZERO
                    };
                    if deficit > Decimal::ZERO {
                        deficits.push((
                            target.asset_class.clone(),
                            deficit,
                            from_weight,
                            target.target_weight,
                        ));
                    }
                }

                let total_deficit: Decimal = deficits.iter().map(|(_, d, _, _)| *d).sum();

                if total_deficit > Decimal::ZERO {
                    // Distribute contribution proportionally to deficits.
                    let available = contribution.min(total_deficit);
                    for (asset_class, deficit, from_weight, _target_weight) in &deficits {
                        let alloc = *deficit / total_deficit * available;
                        if alloc >= min_trade_amount {
                            let new_value = class_values
                                .get(asset_class)
                                .copied()
                                .unwrap_or(Decimal::ZERO)
                                + alloc;
                            let to_weight = if new_total > Decimal::ZERO {
                                new_value / new_total
                            } else {
                                Decimal::ZERO
                            };
                            suggestions.push(RebalanceSuggestion {
                                asset_class: asset_class.clone(),
                                action: "buy".to_string(),
                                amount: alloc,
                                from_weight: *from_weight,
                                to_weight,
                            });
                        }
                    }
                    if total_deficit > contribution {
                        contribution_needed = total_deficit - contribution;
                    }
                }

                RebalanceResult {
                    suggestions,
                    total_portfolio_value: new_total,
                    contribution_needed,
                }
            }
            RebalanceStrategy::ThresholdOnly => {
                // Only rebalance asset classes exceeding tolerance.
                let drift_result = Self::allocation_drift(holdings, targets);
                let new_total = current_total + contribution;
                let mut suggestions = Vec::new();

                for alloc in &drift_result.allocations {
                    if !alloc.exceeds_band {
                        continue;
                    }
                    let target_value = alloc.target_weight * new_total;
                    let diff = target_value - alloc.current_value;

                    if diff.abs() >= min_trade_amount {
                        let (action, amount) = if diff > Decimal::ZERO {
                            ("buy".to_string(), diff)
                        } else {
                            ("sell".to_string(), diff.abs())
                        };
                        suggestions.push(RebalanceSuggestion {
                            asset_class: alloc.asset_class.clone(),
                            action,
                            amount,
                            from_weight: alloc.current_weight,
                            to_weight: alloc.target_weight,
                        });
                    }
                }

                RebalanceResult {
                    suggestions,
                    total_portfolio_value: new_total,
                    contribution_needed: Decimal::ZERO,
                }
            }
        }
    }
}
