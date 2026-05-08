# Plan 0004 — N/A display for zero-count metric rows

## Problem

When a language (or entity type) has TP=0, FP=0, FN=0 — e.g. English samples with
no expected PII and no false positives — `compute_metrics` returns precision=recall=f1=f2=0.0.
The terminal shows them in RED (misleading: looks like a failure) and the JSON shows 0.0
(misleading: looks like measured-zero rather than "undefined").

## Goal

When TP=FP=FN=0, treat precision/recall/f1/f2 as **undefined**:
- Terminal: show `N/A` in DIM instead of colored `0.000`
- JSON: emit `null` instead of `0.0`

Counts (TP/FP/FN) remain visible and non-null in all cases.

## Scope

Three files: `src/model.rs`, `src/stats.rs`, `src/report.rs`.
No changes to `checker.rs`, `main.rs`, `presidio_client.rs`.

## Steps

1. **`model.rs`** — change `MetricSet` metric fields from `f64` to `Option<f64>`:
   ```rust
   pub struct MetricSet {
       pub tp: usize,
       pub fp: usize,
       pub r#fn: usize,
       pub precision: Option<f64>,
       pub recall:    Option<f64>,
       pub f1:        Option<f64>,
       pub f2:        Option<f64>,
   }
   ```
   `serde` serialises `Option<f64>` as `null` when `None` automatically.

2. **`stats.rs`** — update `compute_metrics`: if `tp + fp + fn_ == 0` return all
   metric fields as `None`; otherwise compute and return `Some(value)` as before.
   Update the 7 unit tests to unwrap `Option<f64>` where needed.

3. **`report.rs`** — update `print_metric_row`: if any metric is `None` (i.e.
   tp=fp=fn=0), print `P=N/A  R=N/A  F1=N/A  F2=N/A` in DIM.
   Otherwise behaviour is identical to today.

## Expected output (after)

Terminal:
```
By language
  en
    strict      P=N/A   R=N/A   F1=N/A   F2=N/A   TP=0 FP=0 FN=0
    relaxed     P=N/A   R=N/A   F1=N/A   F2=N/A   TP=0 FP=0 FN=0
  it
    strict      P=0.571  R=1.000  F1=0.727  F2=0.870  TP=4 FP=3 FN=0
```

JSON:
```json
"en": {
  "strict":  { "tp": 0, "fp": 0, "fn": 0, "precision": null, "recall": null, "f1": null, "f2": null },
  "relaxed": { "tp": 0, "fp": 0, "fn": 0, "precision": null, "recall": null, "f1": null, "f2": null }
}
```

## Out of scope

- Hiding zero rows entirely (plan option 2 — not chosen)
- Changing checker or main orchestration logic
