# pii-eval

A Rust CLI tool that evaluates PII detection quality against a labelled test dataset.

It supports two backends:
- **[Microsoft Presidio](https://microsoft.github.io/presidio/)** — rule-based NLP analyzer
- **[Ollama](https://ollama.com/)** — local LLM (e.g. `qwen2.5:7b-instruct`) guided by a structured system prompt

It sends each test sample to the configured backend, compares the response to the expected entities, accumulates precision/recall/F1/F2 metrics, and outputs a full report — both to the terminal (ANSI colored) and to a JSON file.

---

## Prerequisites

- **Rust** stable toolchain (`rustup install stable`)
- **Git** (used by `build.rs` to embed the commit SHA as version string)
- **Presidio Analyzer** — if using `--backend presidio` (see [Running Presidio locally](#running-presidio-locally))
- **Ollama** — if using `--backend ollama` (see [Running Ollama locally](#running-ollama-locally))

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
| `--analyzer-url <URL>` | `http://localhost:5002/analyze` | Analyzer endpoint — Presidio URL or Ollama `/api/chat` URL depending on `--backend` |
| `--output <PATH>` | auto-generated | Destination for the JSON report. If omitted, the file is named `pii-eval-{version}-{YYYY-MM-DD_HH_mm_SS}.json` |
| `--recursive` | `false` | Walk `--input` recursively |
| `--verbose`, `-v` | `false` | Print per-sample TP/FP/FN counts while running |
| `--backend <NAME>` | `presidio` | Backend to use: `presidio` or `ollama` |
| `--ollama-model <MODEL>` | — | Ollama model name — **required** when `--backend ollama` |
| `--system-prompt <PATH>` | `prompts/v1.md` | Path to the LLM system prompt file (Ollama only) |
| `--timeout-secs <N>` | `120` | HTTP timeout in seconds applied to all analyzer requests |
| `--warmup-timeout-secs <N>` | `300` | Timeout in seconds for the warm-up request sent before the eval loop (Ollama only) |

All parameters (including defaults) are recorded in the JSON report under `params` for full reproducibility.

### Examples

```bash
# Presidio backend (default)
pii-eval --input ./test-data --output my-report.json --verbose

# Ollama backend
pii-eval --backend ollama \
         --analyzer-url http://localhost:11434/api/chat \
         --ollama-model qwen2.5:7b-instruct-q4_K_M \
         --input ./test-data --verbose
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
      "expected": [
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
| `expected` | array | List of expected PII spans |
| `expected[].entity_type` | string | Entity type (e.g. `PERSON`, `EMAIL_ADDRESS`) |
| `expected[].start` | integer | Character offset of the span start (inclusive) |
| `expected[].end` | integer | Character offset of the span end (exclusive) |

> **`presidio_expected` is still accepted** as an alias for backward compatibility with existing test files.

> **Why no `text` inside `expected`?**
> The match logic works entirely on `entity_type + start + end`.
> The text of the detected span is extracted from the sample `text` field on the fly when needed for display.
> Storing it redundantly in the expected list would be a source of inconsistency and confusion.

### Multiple files

You can split your test cases across as many files as you want.
`pii-eval` reads **one file at a time** — it never loads the full dataset into memory.
Each file is parsed, processed, and dropped before the next one is opened.

---

## Adding your own test cases

Test data is the heart of this tool — the more varied and realistic the samples, the more meaningful the evaluation. **You are encouraged to add your own test files** covering your language, domain, and entity types. No code changes needed: just drop a `.json` file in the `--input` folder and run.

### Step 1 — Create a test file

Create a `.json` file anywhere inside your input folder (default: `test-data/`). The filename becomes the prefix of every sample ID in the report, so choose something descriptive:

```
test-data/
  italian_names.json
  english_addresses.json
  no_pii_legal_text.json
  edge_cases_email.json
```

### Step 2 — Write your samples

Each file is a JSON object with a `samples` array. Every sample needs four fields:

```json
{
  "samples": [
    {
      "id": "001",
      "lang": "it",
      "text": "Il dott. Luca Ferri abita in via Roma 12, Torino.",
      "expected": [
        { "entity_type": "PERSON",   "start": 9,  "end": 20 },
        { "entity_type": "LOCATION", "start": 41, "end": 47 }
      ]
    }
  ]
}
```

If the text contains **no PII at all**, set `expected` to an empty array — any detection by Presidio will be reported as a false positive:

```json
{
  "id": "002",
  "lang": "en",
  "text": "The forest changes colour slowly in autumn, almost without noticing.",
  "expected": []
}
```

### Step 3 — Get the offsets right

Offsets are **Unicode character offsets**: `start` is inclusive, `end` is exclusive. The easiest way to verify them is Python:

```python
text = "Il dott. Luca Ferri abita in via Roma 12, Torino."
print(text[9:20])   # → "Luca Ferri"
print(text[41:47])  # → "Torino"
```

A quick way to find offsets for any substring:

```python
text = "your full sample text here"
target = "substring to find"
start = text.index(target)
end = start + len(target)
print(f"start={start}, end={end}")
```

> **Tip**: Python `str` indexing uses Unicode code points — the same convention used by pii-eval, Presidio, and the Ollama prompt. `text[start:end]` always gives the expected substring, even for accented Italian text (`è`, `à`, `ù`) or emoji.

### Step 4 — Choose the right entity type

Use the exact Presidio entity type names. Common ones:

| Entity type | Examples |
|-------------|---------|
| `PERSON` | names, surnames |
| `LOCATION` | cities, countries, addresses |
| `EMAIL_ADDRESS` | `mario@example.com` |
| `PHONE_NUMBER` | `+39 02 1234567` |
| `DATE_TIME` | `12/03/1985`, `next Monday` |
| `IBAN_CODE` | `IT60X0542811101000000123456` |
| `CREDIT_CARD` | `4111 1111 1111 1111` |
| `IP_ADDRESS` | `192.168.1.1` |
| `URL` | `https://example.com` |
| `IT_FISCAL_CODE` | `RSSMRA85M01H501Z` |
| `IT_VAT_CODE` | `IT12345678901` |
| `IT_IDENTITY_CARD` | Italian carta d'identità number |
| `IT_PASSPORT` | Italian passport number |
| `US_SSN`, `US_PASSPORT` | US-specific identifiers |
| `NHS_NUMBER` | UK National Health Service number |

The full list depends on your Presidio configuration. Unsupported types will simply never match (all expected become false negatives).

### Tips for good test data

- **One concern per file** — group by scenario (`names_only.json`, `mixed_pii.json`, `no_pii_news_articles.json`). The filename appears in every error line of the report.
- **Cover edge cases** — partial matches, PII embedded in longer strings, consecutive entities, entities at the start or end of the string.
- **Test clean text too** — files with `expected: []` for every sample are valuable: they surface false positives on domain-specific vocabulary (legal, medical, technical).
- **Use realistic text** — synthetic phrases like `"My name is John"` tell you less than actual sentences from your real documents.
- **Mix languages in separate files** — one language per file is not required, but keeping them separate makes the `by_language` section of the report more readable.
- **Keep files small** — a few dozen samples per file is ideal. `pii-eval` loads one file at a time, so there is no memory penalty for having many small files.

---

## Understanding the metrics

### Strict vs Relaxed match

Every expected entity is compared against every Presidio prediction using two criteria:

| Mode | Condition |
|------|-----------|
| **Strict** | `entity_type` equal **and** `start` equal **and** `end` equal |
| **Relaxed** | `entity_type` equal **and** spans overlap by at least 1 character (`pred.start < exp.end && pred.end > exp.start`) |

A **near miss** is a relaxed match that is not a strict match — Presidio found the right entity type and roughly the right place, but the exact character offsets differ.

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

The report differs between backends — `params` is the only section that changes.

**Presidio backend** (no prompt fields):

```json
{
  "version": "a1b2c3d",
  "generated_at": "2026-05-07T10:00:00Z",
  "params": {
    "input": "./test-data",
    "analyzer_url": "http://localhost:5002/analyze",
    "output": "pii-eval-a1b2c3d-2026-05-07_10_00_00.json",
    "recursive": false,
    "verbose": false,
    "backend": "presidio",
    "timeout_secs": 120
  },
  "summary": {
    "files": 3,
    "samples": 42,
    "api_errors": 0,
    "entities_expected": 130,
    "entities_predicted": 125,
    "elapsed": "00:00:08"
  },
  "global": {
    "strict":  { "tp": 115, "fp": 10, "fn": 15, "precision": 0.920, "recall": 0.885, "f1": 0.902, "f2": 0.891 },
    "relaxed": { "tp": 118, "fp":  7, "fn": 12, "precision": 0.944, "recall": 0.908, "f1": 0.926, "f2": 0.914 }
  },
  "by_entity_type": {
    "PERSON": { "strict": { "tp": 50, "fp": 2, "fn": 3, "precision": 0.962, "recall": 0.943, "f1": 0.952, "f2": 0.947 },
                "relaxed": { "tp": 52, "fp": 0, "fn": 1, "precision": 1.0, "recall": 0.981, "f1": 0.990, "f2": 0.985 } }
  },
  "by_language": {
    "it": { "strict": { "tp": 80, "fp": 6, "fn": 10, "precision": 0.930, "recall": 0.889, "f1": 0.909, "f2": 0.897 },
            "relaxed": { "tp": 83, "fp": 3, "fn": 7,  "precision": 0.965, "recall": 0.922, "f1": 0.943, "f2": 0.930 } }
  },
  "test_errors": [
    {
      "sample_id": "italian_names::003",
      "severity": "High",
      "kinds": ["FalseNegative"],
      "near_misses": [],
      "false_positives": [],
      "false_negatives": [
        { "entity_type": "PERSON", "start": 22, "end": 27, "text": "Rossi" }
      ]
    },
    {
      "sample_id": "addresses::007",
      "severity": "Medium",
      "kinds": ["NearMiss"],
      "near_misses": [
        {
          "obtained": { "entity_type": "LOCATION", "start": 5, "end": 11, "text": "Milano" },
          "expected": { "entity_type": "LOCATION", "start": 5, "end": 12, "text": "Milano," }
        }
      ],
      "false_positives": [],
      "false_negatives": []
    }
  ],
  "api_errors": []
}
```

**Ollama backend** — identical structure, with additional prompt fields in `params` and longer `elapsed`:

```json
{
  "version": "a1b2c3d",
  "generated_at": "2026-05-07T10:00:00Z",
  "params": {
    "input": "./test-data",
    "analyzer_url": "http://localhost:11434/api/chat",
    "output": "pii-eval-a1b2c3d-2026-05-07_10_00_00.json",
    "recursive": false,
    "verbose": false,
    "backend": "ollama",
    "ollama_model": "qwen2.5:7b-instruct-q4_K_M",
    "system_prompt_path": "prompts/v1.md",
    "system_prompt_content": "You are a PII detection engine like the Microsoft Presidio Analyzer. [... full prompt omitted in this example]",
    "timeout_secs": 120,
    "warmup_timeout_secs": 300
  },
  "summary": {
    "files": 3,
    "samples": 42,
    "api_errors": 0,
    "entities_expected": 130,
    "entities_predicted": 127,
    "elapsed": "00:12:45"
  },
  "global": { "strict": { "..." }, "relaxed": { "..." } },
  "by_entity_type": { "..." },
  "by_language": { "..." },
  "test_errors": [ "..." ],
  "api_errors": []
}
```

> **Note:** metric values are rounded to 3 decimal places. The actual JSON contains full floating-point precision (e.g. `0.9200000000000001`).

---

## Running Ollama locally

### macOS / Linux — native (recommended)

```bash
# install
brew install ollama          # macOS
# or: curl -fsSL https://ollama.com/install.sh | sh   # Linux

# start the server (runs in background on port 11434)
ollama serve &

# pull the model
ollama pull qwen2.5:7b-instruct-q4_K_M

# verify
curl -s http://localhost:11434/api/tags | python3 -m json.tool
```

### Docker

```bash
# CPU only
docker run -d \
  --name ollama \
  -p 11434:11434 \
  -v ollama_data:/root/.ollama \
  ollama/ollama

# NVIDIA GPU
docker run -d \
  --name ollama \
  --gpus all \
  -p 11434:11434 \
  -v ollama_data:/root/.ollama \
  ollama/ollama

# pull the model inside the container
docker exec ollama ollama pull qwen2.5:7b-instruct-q4_K_M

# verify
curl -s http://localhost:11434/api/tags
```

Then run `pii-eval` pointing to the Ollama endpoint:

```bash
pii-eval --backend ollama \
         --analyzer-url http://localhost:11434/api/chat \
         --ollama-model qwen2.5:7b-instruct-q4_K_M \
         --system-prompt prompts/v1.md \
         --input ./test-data --verbose
```

> **Model not available?** If Ollama returns a "model not found" error, run
> `ollama pull <model-name>` to download it, then retry.

---

## System prompt (Ollama backend)

When using `--backend ollama`, pii-eval sends a **system prompt** to the model before each sample. This prompt defines the task, the output format, and the entity types the model should detect.

### Prompt files

Prompts live in the `prompts/` directory. The current default is `prompts/v1.md`.

To run with a specific prompt:

```bash
pii-eval --backend ollama \
         --analyzer-url http://localhost:11434/api/chat \
         --ollama-model qwen2.5:7b-instruct-q4_K_M \
         --system-prompt prompts/v1.md \
         --input ./test-data
```

### Creating a new prompt variant

Copy the current prompt and edit it:

```bash
cp prompts/v1.md prompts/v2.md
# edit prompts/v2.md
```

Run with the new prompt and compare results:

```bash
pii-eval --system-prompt prompts/v1.md --backend ollama \
         --analyzer-url http://localhost:11434/api/chat \
         --ollama-model qwen2.5:7b-instruct-q4_K_M \
         --input ./test-data --output report-v1.json

pii-eval --system-prompt prompts/v2.md --backend ollama \
         --analyzer-url http://localhost:11434/api/chat \
         --ollama-model qwen2.5:7b-instruct-q4_K_M \
         --input ./test-data --output report-v2.json
```

### Prompts are recorded in the report

Every JSON report includes the **full prompt text** used in that run under `params.system_prompt_content`. Reports are self-contained: you can always reconstruct exactly which prompt produced which results, even after the prompt files have changed.

```json
"params": {
  "system_prompt_path": "prompts/v2.md",
  "system_prompt_content": "You are a PII detection engine..."
}
```

> **Presidio backend**: `--system-prompt` is not used. The `system_prompt_path` and `system_prompt_content` fields are absent from the report.

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
