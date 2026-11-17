//! Nearest-rank quantiles and a repeated-breach rule, adapted from the reference harness.
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
pub(crate) struct Budget {
    pub(crate) metric: String,
    pub(crate) percentile: String,
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
    let breached=budgets.entries.iter().filter(|b|b.metric==metric).any(|b|b.budget.is_some_and(|limit|match b.percentile.as_str(){"p50"=>p50,"max"=>max,_=>p95}>limit));
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
