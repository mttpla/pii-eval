# Plan 0006 — Ollama LLM backend

## Goal

Add a second analyzer backend (Ollama) alongside Presidio, selectable via `--backend`.
The checker, stats, and report pipeline remain unchanged.

---

## ⚠️ Out of scope — address in future plans

**1. Char/byte offset mismatch — latent bug.**
Presidio (Python) returns **character** offsets; Rust treats them as **byte** offsets.
For ASCII test data they coincide; for Italian text with accents (e.g. "è", "à") they diverge
by one byte per multibyte character. Current test data is all ASCII so the bug is invisible.
OllamaClient's text-search fallback is naturally resilient to this.
Presidio on non-ASCII text remains potentially incorrect until fixed.
→ Address in a future plan (no number assigned yet). Must be resolved before adding Italian test data with accented characters.

**2. Ollama model-not-available error handling.**
When the requested model is not loaded in Ollama, the API returns a specific error (e.g. "model not found").
Currently this would surface as a generic `api_error`. A future improvement should detect this condition
and print a clear message suggesting the user run `ollama pull <model-name>` to download or verify the model.
This applies to both missing models and models that exist in the registry but have not been pulled locally.
→ Address in a future plan (no number assigned yet).

---

## Architecture

```d2
direction: right

fs_in: "test-data/*.json" {shape: page}
fs_prompt: "llm-prompt.md\n(system prompt)" {shape: page}
presidio_api: "Presidio\nHTTP /analyze" {shape: cylinder}
ollama_api: "Ollama\nHTTP /api/chat" {shape: cylinder}
stdout: "stdout\n(ANSI colored)" {shape: page}
fs_out: "report.json" {shape: page}

main: "main.rs" {
  description: "CLI · backend selection\nper-file loop"
}
model: "model.rs" {
  description: "shared types\n+ RunParams"
  style.fill: "#eeeeee"
}
analyzer: "analyzer.rs" {
  description: "trait Analyzer"
  style.fill: "#eeeeee"
}
presidio_client: "presidio_client.rs" {
  description: "impl Analyzer\nanalyze(text, lang)"
}
ollama_client: "ollama_client.rs" {
  description: "impl Analyzer\nanalyze(text, lang)\nOption C offset resolution"
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

fs_in -> main: "walkdir\none file at a time"
fs_prompt -> ollama_client: "--system-prompt\n(or fallback ./llm-prompt.md)"
main -> presidio_client: "text, lang\n(--backend presidio)"
main -> ollama_client: "text, lang\n(--backend ollama)"
presidio_client -> presidio_api: "POST /analyze\n{text, language}"
presidio_api -> presidio_client: "[{entity_type, start, end}]"
ollama_client -> ollama_api: "POST /api/chat\n{model, messages, stream:false}"
ollama_api -> ollama_client: "{entities:[{entity_type,text,start,end}]}"
ollama_client -> ollama_client: "Option C: validate offset\n→ fallback text search"
presidio_client -> main: "Result<Vec<PredictedSpan>>"
ollama_client -> main: "Result<Vec<PredictedSpan>>"
main -> checker: "sample_id\npredicted, expected"
checker -> main: "CheckResult\n(SampleCounts + Option<TestError>)"
main -> stats: "add(counts, by_type, lang)"
main -> report: "EvalReport"
report -> stdout
report -> fs_out

model -> main:               {style.stroke-dash: 3; label: "types"}
analyzer -> main:            {style.stroke-dash: 3; label: "trait"}
analyzer -> presidio_client: {style.stroke-dash: 3}
analyzer -> ollama_client:   {style.stroke-dash: 3}
model -> presidio_client:    {style.stroke-dash: 3}
model -> ollama_client:      {style.stroke-dash: 3}
model -> checker:            {style.stroke-dash: 3}
model -> stats:              {style.stroke-dash: 3}
model -> report:             {style.stroke-dash: 3}
```

---

## CLI parameters — all serialized in the report

| Flag | Default | Notes |
|---|---|---|
| `--input` | `./test-data` | unchanged |
| `--analyzer-url` | `http://localhost:5002/analyze` | used by both backends |
| `--output` | auto-generated | unchanged |
| `--recursive` | `false` | unchanged |
| `--verbose` | `false` | unchanged |
| `--backend` | `presidio` | `presidio` or `ollama` |
| `--ollama-model` | — | **required** when `--backend ollama` |
| `--system-prompt` | — | optional; fallback to `./llm-prompt.md` |
| `--timeout-secs` | `120` | applied to both backends |

When `--backend ollama`: if `--ollama-model` is absent → hard error at startup, before the sample loop.

---

## RunParams (model.rs) — all params logged in the report

```rust
pub struct RunParams {
    pub input: String,
    pub analyzer_url: String,
    pub output: String,
    pub recursive: bool,
    pub verbose: bool,
    pub backend: String,
    pub ollama_model: Option<String>,
    pub system_prompt_path: Option<String>,
    pub timeout_secs: u64,
}
```

---

## Tasks

### Task 1 — `analyzer.rs` (new file)

Defines the shared trait. No other module changes in this task.

```rust
use anyhow::Result;
use crate::model::PredictedSpan;

pub trait Analyzer {
    fn analyze(&self, text: &str, lang: &str) -> Result<Vec<PredictedSpan>>;
}
```

### Task 2 — `presidio_client.rs`

- `PresidioClient::new(url: &str, timeout_secs: u64)` — adds timeout via `Client::builder()`
- Remove inherent `pub fn analyze`; move body into `impl Analyzer for PresidioClient`
- No behavioral change

### Task 3 — `llm-prompt.md`

- **Remove** the `score` field from the output schema and all examples
- **Add** `start` and `end`: character offsets, 0-based, exclusive end, consistent with Python `text[start:end]`
- Final schema: `{"entities":[{"entity_type":"...","text":"...","start":N,"end":N}]}`
- Update all three examples: add `start`/`end`, drop `score`
- Add to Rule 2: the LLM must emit a separate entry for each occurrence, each with its own `start`/`end`

### Task 4 — `ollama_client.rs` (new file)

**Struct:**
```rust
pub struct OllamaClient {
    client: Client,       // built with timeout_secs via Client::builder()
    url: String,
    model: String,
    system_prompt: String,
}
```

**Request body — `stream: false` is mandatory and hardcoded.**
Without this flag Ollama responds in streaming mode (one JSON object per token) and the parser crashes.
```json
{
  "model": "<model>",
  "messages": [
    {"role": "system", "content": "<system_prompt>"},
    {"role": "user",   "content": "<text>"}
  ],
  "stream": false
}
```

**Response parsing:**
1. Extract `.message.content` as a string
2. Strip ` ```json ``` ` fences if present (LLM may ignore Rule 7)
3. Parse as `{"entities":[{"entity_type","text","start","end"}]}`
4. If `done_reason != "stop"`: log a warning, attempt parse anyway

**Offset resolution — Option C, per entity in response order:**

```
1. VALIDATE: source.get(start..end) == Some(entity_text)
   → valid: use (start, end) directly

2. FALLBACK: source.match_indices(entity_text)
   → find the first occurrence not overlapping any already-assigned span
   → byte offsets from match_indices are UTF-8-correct

3. NOT FOUND: log a warning for this entity, skip it
   → the sample continues; this is NOT an api_error
```

**Duplicate tracker:** `Vec<(usize, usize)>` of assigned byte ranges, reset on each `analyze()` call.

### Task 5 — `main.rs` + `model.rs`

**model.rs:**
- Update `RunParams` with the new fields (see above)
- Rename field `presidio_expected` → `expected` on `TestSample`, keeping backward compat:
  ```rust
  #[serde(alias = "presidio_expected")]
  pub expected: Vec<ExpectedEntity>,
  ```

**main.rs:**
- Add `mod analyzer;` and `mod ollama_client;`
- Add new `Args` fields for the new flags
- Startup validation for `--backend ollama`:
  ```rust
  let model = args.ollama_model
      .ok_or_else(|| anyhow!("--ollama-model is required when --backend ollama"))?;
  let prompt_path = args.system_prompt
      .unwrap_or_else(|| PathBuf::from("llm-prompt.md"));
  let prompt = fs::read_to_string(&prompt_path)
      .with_context(|| format!("system prompt not found: {}", prompt_path.display()))?;
  ```
- Build `Box<dyn Analyzer>` based on `--backend`:
  ```rust
  let analyzer: Box<dyn Analyzer> = match args.backend {
      Backend::Presidio => Box::new(PresidioClient::new(&args.analyzer_url, args.timeout_secs)),
      Backend::Ollama   => Box::new(OllamaClient::new(&args.analyzer_url, &model, &prompt, args.timeout_secs)),
  };
  ```
- Replace `client.analyze(...)` with `analyzer.analyze(...)`
- Populate the full `RunParams` from all `Args` fields
- Update `sample.presidio_expected` → `sample.expected` in the loop

### Task 6 — update test data JSON files

Rename `"presidio_expected"` → `"expected"` in all JSON files under `test-data/`.
The `serde` alias in `model.rs` keeps backward compat for any file not yet migrated.

---

## Execution order

```
1 (trait) → 2 (presidio impl) → 3 (prompt) → 4 (ollama client) → 5 (main + model) → 6 (json files)
```

Tasks 1–4 can be integrated independently. Task 5 is the integration point. Task 6 is a pure rename, safe to do last.

---

## Verify

```bash
cargo build --release
cargo test

# Presidio regression
cargo run -- --backend presidio --input ./test-data --verbose

# Ollama
cargo run -- --backend ollama \
             --analyzer-url http://localhost:11434/api/chat \
             --ollama-model qwen2.5:7b-instruct-q4_K_M \
             --input ./test-data --verbose
```
