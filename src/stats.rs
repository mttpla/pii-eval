use std::collections::BTreeMap;

use crate::model::{DualMetrics, MetricSet, SampleCounts};

pub struct Stats {
    global: SampleCounts,
    by_type: BTreeMap<String, SampleCounts>,
    by_lang: BTreeMap<String, SampleCounts>,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            global: SampleCounts::default(),
            by_type: BTreeMap::new(),
            by_lang: BTreeMap::new(),
        }
    }

    pub fn add(
        &mut self,
        counts: &SampleCounts,
        by_type: &BTreeMap<String, SampleCounts>,
        lang: &str,
    ) {
        add_to(&mut self.global, counts);
        for (etype, type_counts) in by_type {
            add_to(self.by_type.entry(etype.clone()).or_default(), type_counts);
        }
        add_to(self.by_lang.entry(lang.to_string()).or_default(), counts);
    }

    pub fn finalize(
        &self,
    ) -> (DualMetrics, BTreeMap<String, DualMetrics>, BTreeMap<String, DualMetrics>) {
        let global = to_dual(&self.global);
        let by_type = self.by_type.iter().map(|(k, v)| (k.clone(), to_dual(v))).collect();
        let by_lang = self.by_lang.iter().map(|(k, v)| (k.clone(), to_dual(v))).collect();
        (global, by_type, by_lang)
    }
}

fn add_to(acc: &mut SampleCounts, c: &SampleCounts) {
    acc.tp_strict += c.tp_strict;
    acc.fp_strict += c.fp_strict;
    acc.fn_strict += c.fn_strict;
    acc.tp_relaxed += c.tp_relaxed;
    acc.fp_relaxed += c.fp_relaxed;
    acc.fn_relaxed += c.fn_relaxed;
}

fn to_dual(c: &SampleCounts) -> DualMetrics {
    DualMetrics {
        strict: compute_metrics(c.tp_strict, c.fp_strict, c.fn_strict),
        relaxed: compute_metrics(c.tp_relaxed, c.fp_relaxed, c.fn_relaxed),
    }
}

fn compute_metrics(tp: usize, fp: usize, fn_: usize) -> MetricSet {
    if tp + fp + fn_ == 0 {
        return MetricSet { tp, fp, r#fn: fn_, precision: None, recall: None, f1: None, f2: None };
    }

    let tp_f = tp as f64;
    let fp_f = fp as f64;
    let fn_f = fn_ as f64;

    let precision = if tp + fp == 0 { 0.0 } else { tp_f / (tp_f + fp_f) };
    let recall    = if tp + fn_ == 0 { 0.0 } else { tp_f / (tp_f + fn_f) };

    let f1 = if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    };

    // F2: β=2, weights recall twice as much as precision
    let f2 = if precision + recall == 0.0 {
        0.0
    } else {
        5.0 * precision * recall / (4.0 * precision + recall)
    };

    MetricSet { tp, fp, r#fn: fn_, precision: Some(precision), recall: Some(recall), f1: Some(f1), f2: Some(f2) }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_detection_all_ones() {
        let m = compute_metrics(10, 0, 0);
        assert_eq!(m.precision, Some(1.0));
        assert_eq!(m.recall, Some(1.0));
        assert_eq!(m.f1, Some(1.0));
        assert_eq!(m.f2, Some(1.0));
    }

    #[test]
    fn all_zero_returns_none() {
        // tp=fp=fn=0 → nothing to evaluate → all metrics undefined
        let m = compute_metrics(0, 0, 0);
        assert_eq!(m.precision, None);
        assert_eq!(m.recall, None);
        assert_eq!(m.f1, None);
        assert_eq!(m.f2, None);
    }

    #[test]
    fn only_false_positives() {
        // tp=0, fp=5, fn=0 → precision=0, recall=undefined(→0), f1=0
        let m = compute_metrics(0, 5, 0);
        assert_eq!(m.precision, Some(0.0));
        assert_eq!(m.recall, Some(0.0));
        assert_eq!(m.f1, Some(0.0));
    }

    #[test]
    fn only_false_negatives() {
        // tp=0, fp=0, fn=5 → precision=undefined(→0), recall=0, f1=0
        let m = compute_metrics(0, 0, 5);
        assert_eq!(m.precision, Some(0.0));
        assert_eq!(m.recall, Some(0.0));
        assert_eq!(m.f1, Some(0.0));
    }

    #[test]
    fn f2_rewards_recall_over_precision() {
        // High precision, low recall: P=1.0, R=0.5
        let low_recall = compute_metrics(5, 0, 5);
        // Low precision, high recall: P=0.5, R=1.0
        let high_recall = compute_metrics(5, 5, 0);

        // F2 should be higher when recall is high
        assert!(high_recall.f2.unwrap() > low_recall.f2.unwrap());
        // F1 is symmetric, so it should be equal for these mirrored cases
        let diff_f1 = (low_recall.f1.unwrap() - high_recall.f1.unwrap()).abs();
        assert!(diff_f1 < 1e-9);
    }

    #[test]
    fn stats_accumulates_correctly() {
        let mut stats = Stats::new();

        let c1 = SampleCounts { tp_strict: 2, fp_strict: 1, fn_strict: 0,
                                 tp_relaxed: 2, fp_relaxed: 1, fn_relaxed: 0 };
        let c2 = SampleCounts { tp_strict: 1, fp_strict: 0, fn_strict: 1,
                                 tp_relaxed: 2, fp_relaxed: 0, fn_relaxed: 0 };

        let by_type1: BTreeMap<String, SampleCounts> = [
            ("PERSON".to_string(), c1.clone()),
        ].into();
        let by_type2: BTreeMap<String, SampleCounts> = [
            ("PERSON".to_string(), c2.clone()),
        ].into();

        stats.add(&c1, &by_type1, "it");
        stats.add(&c2, &by_type2, "it");

        let (global, by_type, by_lang) = stats.finalize();

        assert_eq!(global.strict.tp, 3);
        assert_eq!(global.strict.fp, 1);
        assert_eq!(global.strict.r#fn, 1);

        let person = by_type.get("PERSON").unwrap();
        assert_eq!(person.strict.tp, 3);

        let it = by_lang.get("it").unwrap();
        assert_eq!(it.relaxed.tp, 4);
    }
}
