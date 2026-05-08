# Plan 0005 — Total execution time in report and stdout

## Motivation

The tool makes one HTTP call to Presidio per sample. As the test dataset grows,
the total run time could become significant — especially if Presidio is remote,
slow to load NLP models, or if many files are processed. Without a visible
elapsed time, there is no baseline to compare against after optimisations
(batching, concurrency, caching) or regressions (more samples, heavier models).

Showing elapsed time on every run makes performance visible from day one,
without requiring external instrumentation.

## Goal

Record the wall-clock duration of the full run and expose it in:
- **stdout**: shown on the summary line alongside files/samples/entities counts
- **JSON report**: stored in `summary.elapsed` as a human-readable string

## Format

`HH:mm:ss` — readable at a glance, unambiguous, works for runs from 1 second
to several hours.

Examples: `00:00:03`, `00:01:47`, `01:23:05`

## Scope

Three files: `src/model.rs`, `src/main.rs`, `src/report.rs`.
No changes to `checker.rs`, `stats.rs`, `presidio_client.rs`.

## Steps

1. **`model.rs`** — add `elapsed: String` to `ReportSummary`:
   ```rust
   pub struct ReportSummary {
       pub files: usize,
       pub samples: usize,
       pub api_errors: usize,
       pub entities_expected: usize,
       pub entities_predicted: usize,
       pub elapsed: String,
   }
   ```

2. **`main.rs`** — start a `std::time::Instant` at the top of `main`, compute
   elapsed before building the report and format it as `HH:mm:ss`:
   ```rust
   let start = std::time::Instant::now();
   // ... existing orchestration loop ...
   let secs = start.elapsed().as_secs();
   let elapsed = format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60);
   ```
   Pass `elapsed` into `ReportSummary`.

3. **`report.rs`** — extend the existing summary `println!` to include elapsed:
   ```
   files=3  samples=10  expected=4  predicted=7  elapsed=00:00:02
   ```

## Expected output (after)

stdout:
```
pii-eval  version ec687fd  2026-05-08T10:00:00Z
files=3  samples=10  expected=4  predicted=7  elapsed=00:00:02
```

JSON:
```json
"summary": {
  "files": 3,
  "samples": 10,
  "api_errors": 0,
  "entities_expected": 4,
  "entities_predicted": 7,
  "elapsed": "00:00:02"
}
```

## Out of scope

- Per-sample timing
- Network latency breakdown (Presidio vs local processing)
