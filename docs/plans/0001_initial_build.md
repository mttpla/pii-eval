# pii-eval — Implementation Plan

This file tracks the initial build plan.
Each subsequent change plan will live in its own `PLAN-<topic>.md` file.

---

## Status legend

- `[ ]` — not started
- `[~]` — in progress
- `[x]` — done

---

## Plan 0 — Initial build

### Task 1 — Scaffold `[x]`

- `Cargo.toml` with deps: `clap` (derive), `serde` + `serde_json`, `reqwest` (blocking + json), `walkdir`, `anyhow`
- `build.rs`: runs `git rev-parse --short HEAD`, emits `GIT_SHA` env var, fallback `"unknown"`
- Create empty `src/{main,model,presidio_client,checker,stats,report}.rs`

---

### Task 2 — `model.rs` — shared types `[x]`

All data structures used across modules. No logic here.

```
ExpectedEntity   { entity_type: String, start: usize, end: usize }
                 // NO text field — match is on entity_type + start + end only

TestSample       { id: String, lang: String, text: String,
                   presidio_expected: Vec<ExpectedEntity> }

TestFile         { samples: Vec<TestSample> }

PredictedSpan    { entity_type: String, start: usize, end: usize }

ErrorKind        enum { NearMiss, FalsePositive, FalseNegative }
                 // NearMiss   = relaxed match but not strict
                 // FalsePositive = predicted, no relaxed match
                 // FalseNegative = expected, no relaxed match

ErrorSeverity    enum { High, Medium, Low }
                 // High   = at least one FalseNegative
                 // Medium = no FN, but has NearMiss
                 // Low    = only FalsePositives

TestError        { sample_id: String,          // "<file_stem>::<sample_id>"
                   severity: ErrorSeverity,
                   kinds: Vec<ErrorKind>,
                   expected: Vec<ExpectedEntity>,
                   obtained: Vec<PredictedSpan> }

SampleCounts     { tp_strict: usize, fp_strict: usize, fn_strict: usize,
                   tp_relaxed: usize, fp_relaxed: usize, fn_relaxed: usize }

MetricSet        { tp: usize, fp: usize, fn: usize,
                   precision: f64, recall: f64, f1: f64, f2: f64 }

DualMetrics      { strict: MetricSet, relaxed: MetricSet }

ApiError         { sample_id: String,
                   message: String,
                   server_body: Option<String> }

ReportSummary    { files: usize, samples: usize, api_errors: usize,
                   entities_expected: usize, entities_predicted: usize }

EvalReport       { version: String,
                   generated_at: String,
                   summary: ReportSummary,
                   global: DualMetrics,
                   by_entity_type: BTreeMap<String, DualMetrics>,
                   by_language:    BTreeMap<String, DualMetrics>,
                   test_errors: Vec<TestError>,
                   api_errors:  Vec<ApiError> }
```

---

### Task 3 — `presidio_client.rs` `[x]`

HTTP client. Does one thing: send text + language, return spans or error.

```
PresidioClient { client: reqwest::blocking::Client, url: String }

impl PresidioClient {
    fn new(url: &str) -> Self
    fn analyze(&self, text: &str, language: &str) -> Result<Vec<PredictedSpan>>
}
```

- Request body: `{ "text": ..., "language": ... }`
- On HTTP error: `bail!("HTTP {status} — {body}")` — always include raw body
- On network error: propagate with context

---

### Task 4 — `checker.rs` `[x]`

Pure comparison logic. No I/O, no HTTP. Takes predicted + expected, returns counts and optional error record.

```
pub fn check(
    sample_id: &str,
    source_text: &str,
    predicted: &[PredictedSpan],
    expected: &[ExpectedEntity],
) -> (SampleCounts, Option<TestError>)
```

Match rules:
- **Strict**: `entity_type == && start == && end ==`
- **Relaxed**: `entity_type == && pred.start < exp.end && pred.end > exp.start`

Algorithm:
1. For each predicted × expected pair: mark strict and relaxed matches (greedy, first-match)
2. Near miss: predicted with relaxed match but not strict
3. FP: predicted with no relaxed match
4. FN: expected with no relaxed match
5. Build `SampleCounts` from match flags
6. If no errors → return `(counts, None)`
7. Else → determine severity (High > Medium > Low), collect kinds, build `TestError`

Severity priority: High if any FN present, else Medium if any NearMiss, else Low.

---

### Task 5 — `stats.rs` `[x]`

Stateful accumulator. Called once per sample, then finalized once at the end.

```
pub struct Stats { ... }  // internal counters

impl Stats {
    pub fn new() -> Self
    pub fn add(&mut self, counts: &SampleCounts, entity_types: &[String], lang: &str)
    pub fn finalize(&self) -> (DualMetrics, BTreeMap<String, DualMetrics>, BTreeMap<String, DualMetrics>)
                           //   global        by_entity_type               by_language
}

fn compute_metrics(tp: usize, fp: usize, fn_: usize) -> MetricSet
```

`by_entity_type`: accumulate counts keyed by every entity type seen in expected + predicted.
`by_language`: accumulate counts keyed by `sample.lang`.

---

### Task 6 — `main.rs` `[x]`

CLI and orchestration. Thin: delegates everything to modules.

```
#[derive(Parser)]
struct Args {
    --input       PathBuf          default: "./test-data"
    --analyzer-url String          default: "http://localhost:5002/analyze"
    --output      PathBuf          default: "presidio_eval_report.json"
    --recursive   bool             default: false
    --verbose/-v  bool             default: false
}
```

Loop (per file, then per sample):

```
walk dir (walkdir, filter .json)
for each file path:
    raw = fs::read_to_string(path)
    test_file = serde_json::from_str::<TestFile>(&raw)
    drop(raw)                          // free the string immediately
    for each mut sample in test_file.samples:
        sample.id = "<stem>::<sample_id>"
        match client.analyze(&sample.text, &sample.lang):
            Ok(predicted) ->
                entity_types = union of types in expected + predicted
                (counts, maybe_err) = checker::check(&sample.id, &sample.text, &predicted, &sample.presidio_expected)
                stats.add(&counts, &entity_types, &sample.lang)
                if let Some(err) = maybe_err -> test_errors.push(err)
                entities_expected += sample.presidio_expected.len()
                entities_predicted += predicted.len()
            Err(e) ->
                eprintln!("ERROR {}", e)
                api_errors.push(ApiError { ... })
    // test_file dropped here

(global, by_type, by_lang) = stats.finalize()
report = EvalReport { version: VERSION, ... }
report::print_console(&report)
report::write_json(&report, &args.output)
eprintln!("Report saved to {}", args.output.display())
if !api_errors.is_empty() -> eprintln!("{} API error(s)", api_errors.len())
```

`VERSION = env!("GIT_SHA")` — set by `build.rs`.

---

### Task 7 — `report.rs` + `README.md` + D2 diagram `[x]`

**`report.rs`**:
- `pub fn print_console(report: &EvalReport)` — ANSI colored table
- `pub fn write_json(report: &EvalReport, path: &Path) -> Result<()>` — pretty JSON

Color thresholds (applied to each of P, R, F1, F2 independently):
- ≥ 0.85 → green
- ≥ 0.60 → yellow
- < 0.60 → red

Console sections:
1. Header: version + timestamp
2. Summary line: files / samples / expected / predicted
3. Global: strict + relaxed row
4. By entity type (if any)
5. By language (if any)
6. Test errors with severity tag `[HIGH]`/`[MEDIUM]`/`[LOW]` and detail lines
7. API errors (if any)

**`README.md`**: written — see `README.md` in repo root.

**D2 diagram** (save as `docs/architecture.d2`):

```d2
direction: right

fs_in: "test-data/*.json" {shape: page}
presidio_api: "Presidio\nHTTP /analyze" {shape: cylinder}
stdout: "stdout\n(ANSI colored)" {shape: page}
fs_out: "report.json" {shape: page}

main: "main.rs" {
  description: "CLI · per-file loop"
}
model: "model.rs" {
  description: "shared types"
  style.fill: "#eeeeee"
}
client: "presidio_client.rs" {
  description: "analyze(text, lang)"
}
checker: "checker.rs" {
  description: "check() → SampleCounts\n+ Option<TestError>"
}
stats: "stats.rs" {
  description: "add() · finalize()\n→ DualMetrics"
}
report: "report.rs" {
  description: "print_console()\nwrite_json()"
}

# runtime data flow
fs_in -> main: "walkdir\none file at a time"
main -> client: "text, lang"
client -> presidio_api: "POST /analyze\n{text, language}"
presidio_api -> client: "Vec<PresidioSpan>"
client -> main: "Result<Vec<PredictedSpan>>"
main -> checker: "sample_id, text\npredicted, expected"
checker -> main: "(SampleCounts,\nOption<TestError>)"
main -> stats: "add(counts, types, lang)"
main -> report: "EvalReport"
report -> stdout
report -> fs_out

# compile-time type dependencies (dashed)
model -> main: {style.stroke-dash: 3; label: "types"}
model -> client: {style.stroke-dash: 3}
model -> checker: {style.stroke-dash: 3}
model -> stats: {style.stroke-dash: 3}
model -> report: {style.stroke-dash: 3}
```

---

## Design decisions log

| Decision | Rationale |
|----------|-----------|
| No `text` in `ExpectedEntity` | Match is on type + offsets; text extracted from sample on the fly for display only |
| One file at a time in memory | Avoids loading the full dataset; caller controls file sizes |
| `checker.rs` separate from `presidio_client.rs` | Pure logic vs I/O — makes checker independently testable |
| `stats.rs` separate accumulator | Keeps main.rs thin; stats state is isolated |
| Full Presidio body on HTTP error | Calls are local; verbosity is acceptable and aids debugging |
| Severity: High > Medium > Low priority | FN = compliance risk; NearMiss = offset issue; FP = nuisance |
| F2 alongside F1 | Recall matters more than Precision for PII leak prevention |
| Git SHA as version | Ties binary to commit without semantic versioning overhead |
