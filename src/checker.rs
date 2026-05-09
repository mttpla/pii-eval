use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    ErrorKind, ErrorSeverity, ExpectedEntity, NearMiss, PredictedSpan, SampleCounts,
    SpanWithText, TestError,
};

pub struct CheckResult {
    pub counts: SampleCounts,
    pub by_type: BTreeMap<String, SampleCounts>,
    pub error: Option<TestError>,
}

pub fn check(
    sample_id: &str,
    source_text: &str,
    predicted: &[PredictedSpan],
    expected: &[ExpectedEntity],
) -> CheckResult {
    let state = run_matching(predicted, expected);
    let counts = global_counts(&state, predicted.len(), expected.len());
    let by_type = per_type_counts_map(predicted, expected, &state);
    let error = build_error(sample_id, source_text, predicted, expected, &state, &counts);
    CheckResult { counts, by_type, error }
}

// ── Matching ───────────────────────────────────────────────────────────────────

struct MatchState {
    strict_pred: Vec<bool>,
    strict_exp: Vec<bool>,
    relaxed_pred: Vec<bool>,
    relaxed_exp: Vec<bool>,
    // relaxed_pair[pi] = Some(ei) when predicted[pi] relaxed-matches expected[ei]
    relaxed_pair: Vec<Option<usize>>,
}

fn run_matching(predicted: &[PredictedSpan], expected: &[ExpectedEntity]) -> MatchState {
    let mut state = MatchState {
        strict_pred: vec![false; predicted.len()],
        strict_exp: vec![false; expected.len()],
        relaxed_pred: vec![false; predicted.len()],
        relaxed_exp: vec![false; expected.len()],
        relaxed_pair: vec![None; predicted.len()],
    };

    for (pi, pred) in predicted.iter().enumerate() {
        for (ei, exp) in expected.iter().enumerate() {
            if !state.strict_exp[ei] && is_strict(pred, exp) {
                state.strict_pred[pi] = true;
                state.strict_exp[ei] = true;
            }
            if state.relaxed_pair[pi].is_none() && !state.relaxed_exp[ei] && is_relaxed(pred, exp)
            {
                state.relaxed_pair[pi] = Some(ei);
                state.relaxed_pred[pi] = true;
                state.relaxed_exp[ei] = true;
            }
        }
    }

    state
}

fn is_strict(pred: &PredictedSpan, exp: &ExpectedEntity) -> bool {
    pred.entity_type == exp.entity_type && pred.start == exp.start && pred.end == exp.end
}

fn is_relaxed(pred: &PredictedSpan, exp: &ExpectedEntity) -> bool {
    pred.entity_type == exp.entity_type && pred.start < exp.end && pred.end > exp.start
}

// ── Counts ─────────────────────────────────────────────────────────────────────

fn global_counts(state: &MatchState, n_pred: usize, n_exp: usize) -> SampleCounts {
    SampleCounts {
        tp_strict: state.strict_exp[..n_exp].iter().filter(|&&v| v).count(),
        fp_strict: state.strict_pred[..n_pred].iter().filter(|&&v| !v).count(),
        fn_strict: state.strict_exp[..n_exp].iter().filter(|&&v| !v).count(),
        tp_relaxed: state.relaxed_exp[..n_exp].iter().filter(|&&v| v).count(),
        fp_relaxed: state.relaxed_pred[..n_pred].iter().filter(|&&v| !v).count(),
        fn_relaxed: state.relaxed_exp[..n_exp].iter().filter(|&&v| !v).count(),
    }
}

fn per_type_counts_map(
    predicted: &[PredictedSpan],
    expected: &[ExpectedEntity],
    state: &MatchState,
) -> BTreeMap<String, SampleCounts> {
    let types: BTreeSet<&str> = predicted
        .iter()
        .map(|p| p.entity_type.as_str())
        .chain(expected.iter().map(|e| e.entity_type.as_str()))
        .collect();

    types
        .into_iter()
        .map(|etype| (etype.to_string(), type_counts(etype, predicted, expected, state)))
        .collect()
}

fn type_counts(
    etype: &str,
    predicted: &[PredictedSpan],
    expected: &[ExpectedEntity],
    state: &MatchState,
) -> SampleCounts {
    SampleCounts {
        tp_strict: state
            .strict_exp.iter().zip(expected)
            .filter(|(&v, e)| v && e.entity_type == etype).count(),
        fp_strict: state
            .strict_pred.iter().zip(predicted)
            .filter(|(&v, p)| !v && p.entity_type == etype).count(),
        fn_strict: state
            .strict_exp.iter().zip(expected)
            .filter(|(&v, e)| !v && e.entity_type == etype).count(),
        tp_relaxed: state
            .relaxed_exp.iter().zip(expected)
            .filter(|(&v, e)| v && e.entity_type == etype).count(),
        fp_relaxed: state
            .relaxed_pred.iter().zip(predicted)
            .filter(|(&v, p)| !v && p.entity_type == etype).count(),
        fn_relaxed: state
            .relaxed_exp.iter().zip(expected)
            .filter(|(&v, e)| !v && e.entity_type == etype).count(),
    }
}

// ── Error building ─────────────────────────────────────────────────────────────

fn build_error(
    sample_id: &str,
    source_text: &str,
    predicted: &[PredictedSpan],
    expected: &[ExpectedEntity],
    state: &MatchState,
    counts: &SampleCounts,
) -> Option<TestError> {
    let near_misses: Vec<NearMiss> = predicted
        .iter()
        .enumerate()
        .filter(|(i, _)| state.relaxed_pred[*i] && !state.strict_pred[*i])
        .map(|(i, pred)| {
            let ei = state.relaxed_pair[i].unwrap();
            let exp = &expected[ei];
            NearMiss {
                obtained: span_with_text(&pred.entity_type, pred.start, pred.end, source_text),
                expected: span_with_text(&exp.entity_type, exp.start, exp.end, source_text),
            }
        })
        .collect();

    let false_positives: Vec<SpanWithText> = predicted
        .iter()
        .enumerate()
        .filter(|(i, _)| !state.relaxed_pred[*i])
        .map(|(_, p)| span_with_text(&p.entity_type, p.start, p.end, source_text))
        .collect();

    let false_negatives: Vec<SpanWithText> = expected
        .iter()
        .enumerate()
        .filter(|(i, _)| !state.relaxed_exp[*i])
        .map(|(_, e)| span_with_text(&e.entity_type, e.start, e.end, source_text))
        .collect();

    let has_fn = counts.fn_relaxed > 0;
    let has_nm = !near_misses.is_empty();
    let has_fp = !false_positives.is_empty();

    if !has_fn && !has_nm && !has_fp {
        return None;
    }

    let severity = if has_fn {
        ErrorSeverity::High
    } else if has_nm {
        ErrorSeverity::Medium
    } else {
        ErrorSeverity::Low
    };

    let mut kinds = Vec::new();
    if has_nm { kinds.push(ErrorKind::NearMiss); }
    if has_fp { kinds.push(ErrorKind::FalsePositive); }
    if has_fn { kinds.push(ErrorKind::FalseNegative); }

    Some(TestError { sample_id: sample_id.to_string(), severity, kinds, near_misses, false_positives, false_negatives })
}

fn span_with_text(entity_type: &str, start: usize, end: usize, source: &str) -> SpanWithText {
    let start_byte = crate::unicode::char_to_byte(source, start);
    let end_byte   = crate::unicode::char_to_byte(source, end);
    SpanWithText {
        entity_type: entity_type.to_string(),
        start,
        end,
        text: source.get(start_byte..end_byte).unwrap_or("").to_string(),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn pred(etype: &str, start: usize, end: usize) -> PredictedSpan {
        PredictedSpan { entity_type: etype.to_string(), start, end }
    }

    fn exp(etype: &str, start: usize, end: usize) -> ExpectedEntity {
        ExpectedEntity { entity_type: etype.to_string(), start, end }
    }

    #[test]
    fn perfect_match_produces_no_error() {
        let result = check("t::1", "", &[pred("PERSON", 10, 20)], &[exp("PERSON", 10, 20)]);
        assert_eq!(result.counts.tp_strict, 1);
        assert_eq!(result.counts.fp_strict, 0);
        assert_eq!(result.counts.fn_strict, 0);
        assert!(result.error.is_none());
    }

    #[test]
    fn empty_predicted_all_false_negatives() {
        let result = check("t::1", "", &[], &[exp("PERSON", 10, 20), exp("LOCATION", 30, 40)]);
        assert_eq!(result.counts.fn_relaxed, 2);
        assert_eq!(result.counts.tp_relaxed, 0);
        let err = result.error.unwrap();
        assert_eq!(err.severity, ErrorSeverity::High);
        assert!(err.kinds.contains(&ErrorKind::FalseNegative));
        assert_eq!(err.false_negatives.len(), 2);
    }

    #[test]
    fn empty_expected_all_false_positives() {
        let result = check("t::1", "", &[pred("PERSON", 10, 20)], &[]);
        assert_eq!(result.counts.fp_relaxed, 1);
        let err = result.error.unwrap();
        assert_eq!(err.severity, ErrorSeverity::Low);
        assert!(err.kinds.contains(&ErrorKind::FalsePositive));
        assert_eq!(err.false_positives.len(), 1);
    }

    #[test]
    fn overlapping_span_is_near_miss() {
        let result = check("t::1", "", &[pred("PERSON", 10, 21)], &[exp("PERSON", 10, 20)]);
        assert_eq!(result.counts.tp_strict, 0);
        assert_eq!(result.counts.tp_relaxed, 1);
        let err = result.error.unwrap();
        assert_eq!(err.severity, ErrorSeverity::Medium);
        assert!(err.kinds.contains(&ErrorKind::NearMiss));
        assert_eq!(err.near_misses.len(), 1);
        assert_eq!(err.near_misses[0].obtained.end, 21);
        assert_eq!(err.near_misses[0].expected.end, 20);
    }

    #[test]
    fn same_span_wrong_type_is_fp_and_fn() {
        let result = check("t::1", "", &[pred("LOCATION", 10, 20)], &[exp("PERSON", 10, 20)]);
        assert_eq!(result.counts.fp_relaxed, 1);
        assert_eq!(result.counts.fn_relaxed, 1);
        let err = result.error.unwrap();
        assert_eq!(err.severity, ErrorSeverity::High);
    }

    #[test]
    fn fn_severity_beats_near_miss() {
        let result = check(
            "t::1", "",
            &[pred("PERSON", 10, 21)],
            &[exp("PERSON", 10, 20), exp("LOCATION", 50, 60)],
        );
        let err = result.error.unwrap();
        assert_eq!(err.severity, ErrorSeverity::High);
        assert!(err.kinds.contains(&ErrorKind::NearMiss));
        assert!(err.kinds.contains(&ErrorKind::FalseNegative));
    }

    #[test]
    fn greedy_prevents_double_matching() {
        let result = check(
            "t::1", "",
            &[pred("PERSON", 10, 20), pred("PERSON", 10, 20)],
            &[exp("PERSON", 10, 20)],
        );
        assert_eq!(result.counts.tp_strict, 1);
        assert_eq!(result.counts.fp_strict, 1);
    }

    #[test]
    fn per_type_counts_are_isolated() {
        let result = check(
            "t::1", "",
            &[pred("PERSON", 10, 20)],
            &[exp("PERSON", 10, 20), exp("LOCATION", 30, 40)],
        );
        let person = result.by_type.get("PERSON").unwrap();
        assert_eq!(person.tp_relaxed, 1);
        assert_eq!(person.fn_relaxed, 0);
        let location = result.by_type.get("LOCATION").unwrap();
        assert_eq!(location.tp_relaxed, 0);
        assert_eq!(location.fn_relaxed, 1);
    }

    #[test]
    fn extracted_text_matches_source() {
        let source = "Mi chiamo Mario Rossi e abito a Milano";
        let result = check(
            "t::1", source,
            &[pred("PERSON", 10, 21)],
            &[exp("PERSON", 10, 21)],
        );
        assert!(result.error.is_none());

        // near miss: extracted text should reflect actual source chars
        let result2 = check(
            "t::1", source,
            &[pred("PERSON", 10, 22)],   // one char too long
            &[exp("PERSON", 10, 21)],
        );
        let err = result2.error.unwrap();
        assert_eq!(err.near_misses[0].obtained.text, "Mario Rossi ");
        assert_eq!(err.near_misses[0].expected.text, "Mario Rossi");

        // FP text extraction
        let result3 = check("t::1", source, &[pred("LOCATION", 32, 38)], &[]);
        let err3 = result3.error.unwrap();
        assert_eq!(err3.false_positives[0].text, "Milano");
    }

    #[test]
    fn extracted_text_multibyte_chars() {
        // "Mi chiamo Léa Dubois"
        // char layout: M(0)i(1) (2)c(3)h(4)i(5)a(6)m(7)o(8) (9)L(10)é(11)a(12) (13)D(14)u(15)b(16)o(17)i(18)s(19)
        // "Léa" = chars 10..13, bytes 10..14 (é is 2 bytes)
        // "Dubois" = chars 14..20, bytes 15..21
        let source = "Mi chiamo Léa Dubois";

        // FP: predicted "Léa" at char 10..13
        let r = check("t::1", source, &[pred("PERSON", 10, 13)], &[]);
        let err = r.error.unwrap();
        assert_eq!(err.false_positives[0].text, "Léa");

        // FP: predicted "Dubois" at char 14..20
        let r2 = check("t::1", source, &[pred("LOCATION", 14, 20)], &[]);
        let err2 = r2.error.unwrap();
        assert_eq!(err2.false_positives[0].text, "Dubois");

        // NearMiss: predicted chars 10..20 "Léa Dubois", expected chars 14..20 "Dubois"
        let r3 = check(
            "t::1", source,
            &[pred("PERSON", 10, 20)],
            &[exp("PERSON", 14, 20)],
        );
        let err3 = r3.error.unwrap();
        assert_eq!(err3.near_misses[0].obtained.text, "Léa Dubois");
        assert_eq!(err3.near_misses[0].expected.text, "Dubois");
    }
}
