//! Provider-neutral accumulation of reported usage across native tool rounds.

use crate::inference::Usage;

/// Adds provider-reported usage and cost across requests in one logical generation.
pub(crate) fn merge_usage(current: Option<Usage>, next: Option<Usage>) -> Option<Usage> {
    match (current, next) {
        (None, None) => None,
        (current, next) => {
            let current = current.unwrap_or_default();
            let next = next.unwrap_or_default();
            Some(Usage {
                input_tokens: merge_count(current.input_tokens, next.input_tokens),
                output_tokens: merge_count(current.output_tokens, next.output_tokens),
                cost_usd: merge_cost(current.cost_usd, next.cost_usd),
            })
        }
    }
}

/// Adds optional provider costs without inventing a value when both rounds omitted it.
fn merge_cost(current: Option<f64>, next: Option<f64>) -> Option<f64> {
    match (current, next) {
        (None, None) => None,
        (current, next) => Some(current.unwrap_or_default() + next.unwrap_or_default()),
    }
}

/// Saturating-adds optional provider counts without inventing a value both rounds omitted.
fn merge_count(current: Option<u64>, next: Option<u64>) -> Option<u64> {
    match (current, next) {
        (None, None) => None,
        (current, next) => Some(
            current
                .unwrap_or_default()
                .saturating_add(next.unwrap_or_default()),
        ),
    }
}
