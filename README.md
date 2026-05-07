# pii-eval

A Rust CLI tool that evaluates [Microsoft Presidio](https://microsoft.github.io/presidio/) PII detection quality against a labelled test dataset.

It sends each test sample to a running Presidio analyzer, compares the response to the expected entities, accumulates precision/recall/F1/F2 metrics, and outputs a full report — both to the terminal (ANSI colored) and to a JSON file.

---

## Prerequisites

- **Rust** stable toolchain (`rustup install stable`)
- **Presidio Analyzer** reachable at an HTTP endpoint (see [Running Presidio locally](#running-presidio-locally))
- **Git** (used by `build.rs` to embed the commit SHA as version string)

---

## Build

```bash
cargo build --release
# binary at: ./target/release/pii-eval
```

---

## Usage

```bash
pii-eval [OPTIONS]
```

### CLI arguments

| Argument | Default | Description |
|----------|---------|-------------|
| `--input <PATH>` | `./test-data` | Folder containing `.json` test files |
| `--analyzer-url <URL>` | `http://localhost:5002/analyze` | Presidio Analyzer HTTP endpoint |
| `--output <PATH>` | `presidio_eval_report.json` | Destination for the JSON report |
| `--recursive` | `false` | Walk `--input` recursively |
| `--verbose`, `-v` | `false` | Print per-sample TP/FP/FN counts while running |

### Example

```bash
# run against local Presidio, read ./test-data, write report to my-report.json
pii-eval --input ./test-data --output my-report.json --verbose
```

---

## Test file format

Each `.json` file inside `--input` must follow this schema:

```json
{
  "samples": [
    {
      "id": "001",
      "lang": "it",
      "text": "Mi chiamo Mario Rossi e abito a Milano",
      "presidio_expected": [
        { "entity_type": "PERSON",   "start": 10, "end": 21 },
        { "entity_type": "LOCATION", "start": 32, "end": 38 }
      ]
    }
  ]
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier within the file. Will appear in the report as `<filename>::<id>` |
| `lang` | string | BCP-47 language code passed to Presidio (e.g. `it`, `en`) |
| `text` | string | The raw text to analyze |
| `presidio_expected` | array | List of expected PII spans |
| `presidio_expected[].entity_type` | string | Presidio entity type (e.g. `PERSON`, `EMAIL_ADDRESS`) |
| `presidio_expected[].start` | integer | Byte offset of the span start (inclusive) |
| `presidio_expected[].end` | integer | Byte offset of the span end (exclusive) |

> **Why no `text` inside `presidio_expected`?**
> The match logic works entirely on `entity_type + start + end`.
> The text of the detected span is extracted from the sample `text` field on the fly when needed for display.
> Storing it redundantly in the expected list would be a source of inconsistency and confusion.

### Multiple files

You can split your test cases across as many files as you want.
`pii-eval` reads **one file at a time** — it never loads the full dataset into memory.
Each file is parsed, processed, and dropped before the next one is opened.

---

## Understanding the metrics

### Strict vs Relaxed match

Every expected entity is compared against every Presidio prediction using two criteria:

| Mode | Condition |
|------|-----------|
| **Strict** | `entity_type` equal **and** `start` equal **and** `end` equal |
| **Relaxed** | `entity_type` equal **and** spans overlap by at least 1 character (`pred.start < exp.end && pred.end > exp.start`) |

A **near miss** is a relaxed match that is not a strict match — Presidio found the right entity type and roughly the right place, but the exact byte offsets differ.

### TP / FP / FN

| Symbol | Name | Meaning |
|--------|------|---------|
| **TP** | True Positive | Expected entity correctly detected |
| **FP** | False Positive | Presidio detected something that was not expected |
| **FN** | False Negative | Expected entity that Presidio missed entirely |

For **strict** mode: near misses count as both FP and FN (wrong offsets = fail).
For **relaxed** mode: near misses count as TP (close enough = pass).

### Precision / Recall / F1 / F2

```
Precision = TP / (TP + FP)   — of what Presidio flagged, how much was correct?
Recall    = TP / (TP + FN)   — of what should have been found, how much was found?

F1 = 2 · P · R / (P + R)    — harmonic mean, equal weight
F2 = 5 · P · R / (4P + R)   — harmonic mean, double weight on Recall
```

### Why F2? Why bias towards Recall for PII?

In PII detection, a **missed entity (FN) is far more dangerous** than a spurious detection (FP).
Leaking a person's name or address because the analyzer missed it is a compliance failure.
Flagging something that is not PII is a nuisance, not a risk.

F2 makes Recall twice as important as Precision, reflecting this asymmetry.
Use F2 as your primary quality gate; use F1 for a balanced view.

### Color thresholds

The terminal output colors each metric score:

| Color | Threshold | Meaning |
|-------|-----------|---------|
| **Green** | score ≥ 0.85 | High quality — Presidio correctly identifies ≥ 85 % of entities with good precision |
| **Yellow** | 0.60 ≤ score < 0.85 | Acceptable — some misses or false positives, worth investigating |
| **Red** | score < 0.60 | Poor quality — significant detection failures, action required |

These thresholds apply to Precision, Recall, F1, and F2 independently.
A green F1 with a red Recall means the detector is precise but misses too many entities.

---

## Error severity

When a sample has at least one mismatch, a `TestError` is recorded.
Each error carries a **severity** and one or more **kinds**:

### Severity

| Severity | Condition | Why it matters |
|----------|-----------|----------------|
| **High** | Sample has at least one False Negative | An expected PII entity was completely missed — highest compliance risk |
| **Medium** | No FN, but has Near Misses | Entity type found, span slightly off — usually an off-by-one in tokenization |
| **Low** | Only False Positives | Presidio over-detected — a nuisance but not a data-leak risk |

### Kinds

A single sample can carry multiple kinds simultaneously:

| Kind | Description |
|------|-------------|
| `NearMiss` | Relaxed match but not strict (correct type, wrong offsets) |
| `FalsePositive` | Predicted span with no relaxed match at all |
| `FalseNegative` | Expected span with no relaxed match at all |

---

## Output

### Terminal (stdout)

```
pii-eval  version a1b2c3d  2026-05-07T10:00:00Z
files=3 samples=42 expected=130 predicted=127

Global
  strict        P=0.921  R=0.884  F1=0.902  F2=0.891  TP=115 FP=10 FN=15
  relaxed       P=0.945  R=0.907  F1=0.926  F2=0.914  TP=118 FP=7  FN=12

By entity type
  PERSON
    strict      P=0.960  R=0.940  F1=0.950  F2=0.944  ...
    relaxed     P=0.980  R=0.960  F1=0.970  F2=0.964  ...
  ...

By language
  it
    strict      ...
  ...

Samples with errors  (4 samples)
  italian_names::003   [HIGH]
    missed        PERSON   "Rossi"   (start=22 end=27)
  addresses::007   [MEDIUM]
    near miss     LOCATION  obtained: start=5 end=11  expected: start=5 end=12
  ...
```

### JSON report structure

```json
{
  "version": "a1b2c3d",
  "generated_at": "2026-05-07T10:00:00Z",
  "summary": {
    "files": 3,
    "samples": 42,
    "api_errors": 0,
    "entities_expected": 130,
    "entities_predicted": 127
  },
  "global": {
    "strict":  { "tp": 115, "fp": 10, "fn": 15, "precision": 0.921, "recall": 0.884, "f1": 0.902, "f2": 0.891 },
    "relaxed": { "tp": 118, "fp":  7, "fn": 12, "precision": 0.945, "recall": 0.907, "f1": 0.926, "f2": 0.914 }
  },
  "by_entity_type": {
    "PERSON": { "strict": { ... }, "relaxed": { ... } }
  },
  "by_language": {
    "it": { "strict": { ... }, "relaxed": { ... } }
  },
  "test_errors": [
    {
      "sample_id": "italian_names::003",
      "severity": "High",
      "kinds": ["FalseNegative"],
      "expected": [
        { "entity_type": "PERSON", "start": 22, "end": 27 }
      ],
      "obtained": []
    }
  ],
  "api_errors": []
}
```

---

## Running Presidio locally

The fastest way is Docker:

```bash
# pull and start the Presidio Analyzer on port 5002
docker run -d \
  --name presidio-analyzer \
  -p 5002:3000 \
  mcr.microsoft.com/presidio-analyzer:latest

# verify it is up
curl -s http://localhost:5002/health
```

Then run `pii-eval` with the default `--analyzer-url http://localhost:5002/analyze`.

To stop:

```bash
docker stop presidio-analyzer && docker rm presidio-analyzer
```

> **Note on language models**: the default Presidio image ships with English models.
> For Italian (`lang: "it"`) you need a custom image with spaCy `it_core_news_lg` installed,
> or a Presidio deployment configured with the appropriate NLP engine.
