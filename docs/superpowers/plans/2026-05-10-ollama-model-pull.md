# Ollama Model-Not-Found: Actionable Error Message

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When Ollama returns 404 "model not found" during warmup, bail with a clear, actionable message instead of a raw HTTP dump. The early exit at warmup is already correct — only the error text needs improving.

**Architecture:** Single change inside `OllamaClient::warmup()` — detect HTTP 404 with "model not found" body before the generic error path, and emit `"model 'xyz' not found in Ollama — run: ollama pull xyz"`. No new methods, no new args, no new report fields.

**Tech Stack:** Rust, reqwest blocking, mockito (tests), anyhow.

---

## File Map

| File | Change |
|---|---|
| `src/ollama_client.rs` | Detect model-not-found 404 in `warmup()`, emit actionable error |

---

### Task 1: Detect model-not-found in `warmup()` and emit actionable message

**Files:**
- Modify: `src/ollama_client.rs:108-145`

Currently `warmup()` on HTTP 404 emits:
```
warm-up: HTTP 404 from Ollama: {"error":"model 'xyz' not found, try pulling it first"}
```

Target:
```
model 'xyz' not found in Ollama — run: ollama pull xyz
```

Ollama returns HTTP 404 with body `{"error":"model 'xyz' not found, try pulling it first"}` when the model has not been pulled. Detect this before the generic error path.

- [ ] **Step 1: Write failing test**

Add inside `mod warmup_tests` in `src/ollama_client.rs`:

```rust
#[test]
fn warmup_emits_actionable_message_when_model_not_found() {
    let mut server = Server::new();
    let _mock = server
        .mock("POST", "/api/chat")
        .with_status(404)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":"model 'llama3' not found, try pulling it first"}"#)
        .create();

    let url = format!("{}/api/chat", server.url());
    let client = OllamaClient::new(&url, "llama3", "", 5);
    let err = client.warmup(5).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("ollama pull llama3"), "got: {msg}");
    assert!(!msg.contains("HTTP 404"), "should not expose raw HTTP status, got: {msg}");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test warmup_emits_actionable_message_when_model_not_found
```

Expected: FAIL — current code exposes "HTTP 404".

- [ ] **Step 3: Implement the fix in `warmup()`**

In `src/ollama_client.rs`, inside `warmup()`, replace:

```rust
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            anyhow::bail!("warm-up: HTTP {} from Ollama: {}", status, body);
        }
```

With:

```rust
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            if status == 404 && (body.contains("not found") || body.contains("try pulling")) {
                anyhow::bail!(
                    "model '{}' not found in Ollama — run: ollama pull {}",
                    self.model, self.model
                );
            }
            anyhow::bail!("warm-up: HTTP {} from Ollama: {}", status, body);
        }
```

- [ ] **Step 4: Run all warmup tests**

```bash
cargo test warmup
```

Expected: all PASS — new test passes, existing tests (`warmup_succeeds_on_valid_response`, `warmup_fails_on_http_error`, `warmup_fails_on_invalid_json`, `warmup_fails_on_unreachable_url`) still pass.

- [ ] **Step 5: Run full test suite**

```bash
cargo test
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add src/ollama_client.rs
git commit -m "fix(ollama): surface actionable error when model not found during warmup"
```
