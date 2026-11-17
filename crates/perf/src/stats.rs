//! Nearest-rank quantiles and a repeated-breach rule, adapted from the reference harness.
use serde::{Deserialize, Serialize};
/// Accepted quantiles; malformed catalog values fail parsing instead of falling back.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Percentile {
    P50,
    P95,
    P99,
    Max,
}
#[derive(Deserialize)]
pub(crate) struct Budget {
    pub(crate) metric: String,
    pub(crate) percentile: Percentile,
    pub(crate) p50_budget: Option<f64>,
    pub(crate) budget: Option<f64>,
}
#[derive(Deserialize)]
pub(crate) struct Budgets {
    pub(crate) entries: Vec<Budget>,
}
#[derive(Serialize)]
pub(crate) struct Summary {
    pub(crate) metric: String,
    pub(crate) p50: f64,
    pub(crate) p95: f64,
    pub(crate) p99: f64,
    pub(crate) max: f64,
    pub(crate) samples: usize,
    pub(crate) breached: bool,
}
pub(crate) fn quantile(v: &[f64], q: f64) -> f64 {
    v[((v.len() as f64 * q).ceil() as usize).clamp(1, v.len()) - 1]
}
pub(crate) fn summarize(metric: &str, mut samples: Vec<f64>, budgets: &Budgets) -> Summary {
    samples.sort_by(f64::total_cmp);
    let p50 = quantile(&samples, 0.5);
    let p95 = quantile(&samples, 0.95);
    let p99 = quantile(&samples, 0.99);
    let max = *samples.last().unwrap();
    let breached = budgets
        .entries
        .iter()
        .filter(|b| b.metric == metric)
        .any(|b| {
            b.p50_budget.is_some_and(|limit| p50 > limit)
                || b.budget.is_some_and(|limit| {
                    let value = match b.percentile {
                        Percentile::P50 => p50,
                        Percentile::P95 => p95,
                        Percentile::P99 => p99,
                        Percentile::Max => max,
                    };
                    value > limit
                })
        });
    Summary {
        metric: metric.into(),
        p50,
        p95,
        p99,
        max,
        samples: samples.len(),
        breached,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tail_breach_is_not_hidden_by_median() {
        let b: Budgets = serde_json::from_str(
            r#"{"entries":[{"metric":"read","percentile":"p95","budget":50.0}]}"#,
        )
        .unwrap();
        let mut v = vec![1.; 94];
        v.extend([51.; 6]);
        let s = summarize("read", v, &b);
        assert!(s.breached);
        assert_eq!(s.p50, 1.);
        assert_eq!(s.p99, 51.);
    }
}
