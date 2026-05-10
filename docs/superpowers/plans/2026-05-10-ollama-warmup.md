# Ollama Warm-Up Before Eval Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Before the eval loop, send a minimal request to Ollama with a longer configurable timeout so the model is loaded into RAM; fail fast with a clear error if the warm-up itself fails.

**Architecture:** Add `--warmup-timeout-secs` CLI arg (default 300). When `--backend ollama`, call `OllamaClient::warmup(timeout_secs)` inside `build_analyzer` before boxing the client. `warmup()` builds a one-shot `reqwest::blocking::Client` with the longer timeout, sends a trivial chat request (text `"."`), and returns `Err` on any failure. Add `warmup_timeout_secs: Option<u64>` to `RunParams` so reports capture what timeout was used.

**Tech Stack:** Rust, reqwest blocking, clap, serde/serde_json, mockito (dev only)

---

## File map

| File | Change |
|---|---|
| `Cargo.toml` | Add `mockito = "1"` under `[dev-dependencies]` |
| `src/ollama_client.rs` | Add `pub fn warmup(&self, timeout_secs: u64) -> Result<()>` + tests (success, HTTP error, invalid JSON, unreachable URL) |
| `src/main.rs` | Add `warmup_timeout_secs: u64` to `Args`; call warmup in `build_analyzer`; add field to `RunParams` construction; add `#[cfg(test)]` block testing `Args` defaults |
| `src/model.rs` | Add `warmup_timeout_secs: Option<u64>` to `RunParams`; update `base_params` test helper; add two new serialization tests |

---

## Task 1: Add `mockito` dev dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add dev dependency**

After the last line of `Cargo.toml` (currently line 17) add:

```toml
[dev-dependencies]
mockito = "1"
```

- [ ] **Step 2: Verify it resolves**

```bash
cargo test --no-run 2>&1
```

Expected: downloads mockito and compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add mockito dev dependency for HTTP mock tests"
```

---

## Task 2: Add `warmup()` to `OllamaClient`

**Files:**
- Modify: `src/ollama_client.rs`

- [ ] **Step 1: Write the failing tests**

Add a new `mod warmup_tests` block at the very bottom of `src/ollama_client.rs` (after the closing `}` of the existing `mod tests`):

```rust
#[cfg(test)]
mod warmup_tests {
    use super::*;
    use mockito::Server;

    fn valid_response_body() -> &'static str {
        r#"{"message":{"content":""},"done_reason":"stop"}"#
    }

    #[test]
    fn warmup_succeeds_on_valid_response() {
        let mut server = Server::new();
        let mock = server
            .mock("POST", "/api/chat")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(valid_response_body())
            .create();

        let url = format!("{}/api/chat", server.url());
        let client = OllamaClient::new(&url, "test-model", "", 5);
        assert!(client.warmup(5).is_ok());
        mock.assert();
    }

    #[test]
    fn warmup_fails_on_http_error() {
        let mut server = Server::new();
        let _mock = server
            .mock("POST", "/api/chat")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let url = format!("{}/api/chat", server.url());
        let client = OllamaClient::new(&url, "test-model", "", 5);
        let err = client.warmup(5).unwrap_err();
        assert!(err.to_string().contains("warm-up: HTTP 500"));
    }

    #[test]
    fn warmup_fails_on_invalid_json() {
        let mut server = Server::new();
        let _mock = server
            .mock("POST", "/api/chat")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("not json at all")
            .create();

        let url = format!("{}/api/chat", server.url());
        let client = OllamaClient::new(&url, "test-model", "", 5);
        let err = client.warmup(5).unwrap_err();
        assert!(err.to_string().contains("warm-up: cannot parse Ollama response"));
    }

    #[test]
    fn warmup_fails_on_unreachable_url() {
        // Port 19872 is almost certainly unoccupied; refused immediately
        let client = OllamaClient::new("http://127.0.0.1:19872/api/chat", "test-model", "", 1);
        let err = client.warmup(1).unwrap_err();
        assert!(err.to_string().contains("warm-up:"));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

```bash
cargo test warmup_tests 2>&1
```

Expected: compile error — `warmup` method not found on `OllamaClient`.

- [ ] **Step 3: Implement `warmup()`**

Add after the closing `}` of `impl OllamaClient` (around line 107, before `impl Analyzer for OllamaClient`):

```rust
pub fn warmup(&self, timeout_secs: u64) -> Result<()> {
    let warmup_client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .expect("failed to build warmup HTTP client");

    eprintln!("warming up Ollama model '{}' (timeout: {}s)…", self.model, timeout_secs);

    let req = ChatRequest {
        model: &self.model,
        messages: vec![Message { role: "user", content: "." }],
        stream: false,
    };

    let resp = warmup_client
        .post(&self.url)
        .json(&req)
        .send()
        .with_context(|| format!("warm-up: cannot reach Ollama at {}", self.url))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        anyhow::bail!("warm-up: HTTP {} from Ollama: {}", status, body);
    }

    let _: ChatResponse = resp.json().context("warm-up: cannot parse Ollama response")?;

    eprintln!("warm-up complete — model loaded");
    Ok(())
}
```

- [ ] **Step 4: Run all four warmup tests**

```bash
cargo test warmup_tests 2>&1
```

Expected: all 4 pass — `warmup_succeeds_on_valid_response`, `warmup_fails_on_http_error`, `warmup_fails_on_invalid_json`, `warmup_fails_on_unreachable_url`.

- [ ] **Step 5: Run full test suite**

```bash
cargo test 2>&1
```

Expected: all tests pass, no regressions.

- [ ] **Step 6: Commit**

```bash
git add src/ollama_client.rs
git commit -m "feat(ollama): add warmup() with mockito-backed unit tests"
```

---

## Task 3: Add `--warmup-timeout-secs` to `Args` and `RunParams`

**Files:**
- Modify: `src/model.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write failing tests in `model.rs`**

Add inside `#[cfg(test)] mod tests` in `src/model.rs` after the existing two tests:

```rust
#[test]
fn run_params_includes_warmup_timeout_for_ollama() {
    let mut p = base_params("ollama");
    p.warmup_timeout_secs = Some(300);
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains("\"warmup_timeout_secs\""));
    assert!(json.contains("300"));
}

#[test]
fn run_params_omits_warmup_timeout_for_presidio() {
    let p = base_params("presidio");
    let json = serde_json::to_string(&p).unwrap();
    assert!(!json.contains("warmup_timeout_secs"));
}
```

- [ ] **Step 2: Run to verify they fail**

```bash
cargo test run_params_includes_warmup 2>&1
cargo test run_params_omits_warmup 2>&1
```

Expected: compile error — `warmup_timeout_secs` field not found on `RunParams`.

- [ ] **Step 3: Add field to `RunParams` in `src/model.rs`**

After `pub timeout_secs: u64,` (line 144) add:

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warmup_timeout_secs: Option<u64>,
```

- [ ] **Step 4: Update `base_params()` test helper in `src/model.rs`**

Inside `fn base_params`, add to the `RunParams` literal after `timeout_secs: 120,`:

```rust
            warmup_timeout_secs: None,
```

- [ ] **Step 5: Run model tests**

```bash
cargo test --lib model 2>&1
```

Expected: all 4 model tests pass.

- [ ] **Step 6: Write failing Args tests in `src/main.rs`**

Add at the very bottom of `src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn warmup_timeout_defaults_to_300() {
        let args = Args::parse_from(["pii-eval"]);
        assert_eq!(args.warmup_timeout_secs, 300);
    }

    #[test]
    fn warmup_timeout_can_be_overridden() {
        let args = Args::parse_from(["pii-eval", "--warmup-timeout-secs", "60"]);
        assert_eq!(args.warmup_timeout_secs, 60);
    }
}
```

- [ ] **Step 7: Run to verify Args tests fail**

```bash
cargo test warmup_timeout_defaults 2>&1
cargo test warmup_timeout_can_be_overridden 2>&1
```

Expected: compile error — `warmup_timeout_secs` field not found on `Args`.

- [ ] **Step 8: Add `warmup_timeout_secs` to `Args` in `src/main.rs`**

After the `timeout_secs` field in `struct Args` (around line 61):

```rust
    /// Warm-up timeout in seconds for the initial Ollama request (ignored when --backend presidio)
    #[arg(long, default_value_t = 300)]
    warmup_timeout_secs: u64,
```

- [ ] **Step 9: Run Args tests**

```bash
cargo test warmup_timeout 2>&1
```

Expected: `warmup_timeout_defaults_to_300` and `warmup_timeout_can_be_overridden` both pass.

- [ ] **Step 10: Wire `warmup_timeout_secs` into `RunParams` construction in `src/main.rs`**

Before the `let params = RunParams { ... }` block, add:

```rust
    let warmup_timeout_secs = if args.backend == "ollama" {
        Some(args.warmup_timeout_secs)
    } else {
        None
    };
```

Then inside the `RunParams { ... }` struct literal, after `timeout_secs: args.timeout_secs,` add:

```rust
        warmup_timeout_secs,
```

- [ ] **Step 11: Run full test suite**

```bash
cargo test 2>&1
```

Expected: all tests pass.

- [ ] **Step 12: Commit**

```bash
git add src/main.rs src/model.rs
git commit -m "feat: add --warmup-timeout-secs arg and RunParams field"
```

---

## Task 4: Wire warmup call into `build_analyzer`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Call `warmup()` inside the ollama branch of `build_analyzer`**

In `build_analyzer` (around line 221), replace the current ollama branch:

```rust
        "ollama" => {
            let model = args.ollama_model.clone()
                .ok_or_else(|| anyhow::anyhow!("--ollama-model is required when --backend ollama"))?;
            let prompt_path = args.system_prompt.clone()
                .unwrap_or_else(|| PathBuf::from("prompts/v1.md"));
            let prompt = std::fs::read_to_string(&prompt_path)
                .with_context(|| format!("system prompt not found: {}", prompt_path.display()))?;
            let client = OllamaClient::new(&args.analyzer_url, &model, &prompt, args.timeout_secs);
            client.warmup(args.warmup_timeout_secs)?;
            Ok((
                Box::new(client),
                Some(model),
                Some(prompt_path.display().to_string()),
                Some(prompt),
            ))
        }
```

- [ ] **Step 2: Run full test suite**

```bash
cargo test 2>&1
```

Expected: all tests pass. (Unit tests do not invoke `build_analyzer` with the ollama branch live, so no network call occurs.)

- [ ] **Step 3: Build release binary**

```bash
cargo build --release 2>&1
```

Expected: clean build, no warnings.

- [ ] **Step 4: Smoke-test the error path**

```bash
./target/release/pii-eval \
  --backend ollama \
  --ollama-model llama3 \
  --analyzer-url http://127.0.0.1:19872/api/chat \
  --warmup-timeout-secs 2 2>&1 | head -5
```

Expected output (exact lines may vary):
```
warming up Ollama model 'llama3' (timeout: 2s)…
Error: warm-up: cannot reach Ollama at http://127.0.0.1:19872/api/chat
```

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: call OllamaClient::warmup before eval loop"
```

---

## Test coverage summary

| Behaviour | Test | File |
|---|---|---|
| warmup succeeds — valid 200 response | `warmup_succeeds_on_valid_response` | `ollama_client.rs` |
| warmup fails — HTTP 5xx | `warmup_fails_on_http_error` | `ollama_client.rs` |
| warmup fails — invalid JSON body | `warmup_fails_on_invalid_json` | `ollama_client.rs` |
| warmup fails — unreachable URL | `warmup_fails_on_unreachable_url` | `ollama_client.rs` |
| `--warmup-timeout-secs` defaults to 300 | `warmup_timeout_defaults_to_300` | `main.rs` |
| `--warmup-timeout-secs` accepts override | `warmup_timeout_can_be_overridden` | `main.rs` |
| `warmup_timeout_secs` serialized for ollama | `run_params_includes_warmup_timeout_for_ollama` | `model.rs` |
| `warmup_timeout_secs` omitted for presidio | `run_params_omits_warmup_timeout_for_presidio` | `model.rs` |

---

## Self-review

**Spec coverage:**
- `--warmup-timeout-secs` flag with default 300 ✓
- Minimal request before eval loop ✓
- Success → model in RAM, eval proceeds ✓
- Failure → bail early with clear error (`warm-up:` prefix) ✓
- `warmup_timeout_secs` captured in `RunParams` for report ✓

**Placeholder scan:** none found.

**Type consistency:**
- `warmup(timeout_secs: u64)` called as `client.warmup(args.warmup_timeout_secs)` — both `u64` ✓
- `warmup_timeout_secs: Option<u64>` in `RunParams`, set as `Some(args.warmup_timeout_secs)` for ollama / `None` for presidio ✓
- Error prefix `"warm-up:"` matches all three error-path test assertions ✓
- `valid_response_body()` matches `ChatResponse` / `AssistantMessage` struct shapes — `{"message":{"content":""},"done_reason":"stop"}` ✓
