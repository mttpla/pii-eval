# Prompt Content in Report Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Save the full text of the system prompt used in each run into the JSON report, making every report self-contained and reproducible.

**Architecture:** The `--system-prompt` CLI arg and `system_prompt_path` in `RunParams` already exist. Task 1 adds `system_prompt_content: Option<String>` to `RunParams` and threads the prompt text (already read in `build_analyzer`) through to the report. Task 2 creates a `prompts/` directory, moves `llm-prompt.md` there as `v1.md`, and updates the default path in `main.rs`.

**Tech Stack:** Rust, Serde, Clap (no new dependencies).

---

## Background

`src/main.rs::build_analyzer()` already reads the prompt file from disk and passes it to `OllamaClient::new()`. It returns the path as `system_prompt_path` which ends up in `RunParams` and the JSON report. The content is discarded after being handed off to the client. This plan captures it in the report too.

---

## Files

| File | Change |
|---|---|
| `src/model.rs` | Add `system_prompt_content: Option<String>` to `RunParams`; add `skip_serializing_if` to both prompt fields |
| `src/main.rs` | Expand `build_analyzer` return to 4-tuple, thread content into `RunParams` |
| `prompts/v1.md` | New file — copy of current `llm-prompt.md` |
| `llm-prompt.md` | Kept as-is (backward compat for users who rely on it) |

---

## Task 1 — Add `system_prompt_content` to `RunParams` and wire it through

**Files:**
- Modify: `src/model.rs`
- Modify: `src/main.rs`

### Current state of `RunParams` (src/model.rs ~line 130)

```rust
#[derive(Debug, Serialize)]
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

### Current state of `build_analyzer` (src/main.rs ~line 212)

```rust
fn build_analyzer(args: &Args) -> Result<(Box<dyn Analyzer>, Option<String>, Option<String>)> {
    match args.backend.as_str() {
        "presidio" => Ok((
            Box::new(PresidioClient::new(&args.analyzer_url, args.timeout_secs)),
            None,
            None,
        )),
        "ollama" => {
            let model = args.ollama_model.clone()
                .ok_or_else(|| anyhow::anyhow!("--ollama-model is required when --backend ollama"))?;
            let prompt_path = args.system_prompt.clone()
                .unwrap_or_else(|| PathBuf::from("llm-prompt.md"));
            let prompt = std::fs::read_to_string(&prompt_path)
                .with_context(|| format!("system prompt not found: {}", prompt_path.display()))?;
            Ok((
                Box::new(OllamaClient::new(&args.analyzer_url, &model, &prompt, args.timeout_secs)),
                Some(model),
                Some(prompt_path.display().to_string()),
            ))
        }
        other => anyhow::bail!("unknown backend {:?} — use 'presidio' or 'ollama'", other),
    }
}
```

- [ ] **Step 1: Write failing tests**

`src/model.rs` has no `#[cfg(test)]` block yet. Add one at the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn base_params(backend: &str) -> RunParams {
        RunParams {
            input: "./test-data".to_string(),
            analyzer_url: "http://localhost".to_string(),
            output: "out.json".to_string(),
            recursive: false,
            verbose: false,
            backend: backend.to_string(),
            ollama_model: None,
            system_prompt_path: None,
            system_prompt_content: None,
            timeout_secs: 120,
        }
    }

    #[test]
    fn run_params_includes_prompt_content_in_json() {
        let mut p = base_params("ollama");
        p.system_prompt_path = Some("prompts/v1.md".to_string());
        p.system_prompt_content = Some("You are a PII detector.".to_string());
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"system_prompt_content\""));
        assert!(json.contains("You are a PII detector."));
        assert!(json.contains("\"system_prompt_path\""));
    }

    #[test]
    fn run_params_omits_prompt_fields_when_none() {
        let p = base_params("presidio");
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("system_prompt_content"));
        assert!(!json.contains("system_prompt_path"));
    }
}
```

- [ ] **Step 2: Run tests to verify they FAIL**

```bash
cargo test run_params
```
Expected: compile error — `system_prompt_content` field doesn't exist on `RunParams`.

- [ ] **Step 3: Update `RunParams` in `src/model.rs`**

Replace the `RunParams` struct:

```rust
#[derive(Debug, Serialize)]
pub struct RunParams {
    pub input: String,
    pub analyzer_url: String,
    pub output: String,
    pub recursive: bool,
    pub verbose: bool,
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ollama_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_content: Option<String>,
    pub timeout_secs: u64,
}
```

- [ ] **Step 4: Run tests to verify they PASS**

```bash
cargo test run_params
```
Expected: both tests PASS.

- [ ] **Step 5: Update `build_analyzer` in `src/main.rs`**

Replace the function signature and body:

```rust
fn build_analyzer(args: &Args) -> Result<(Box<dyn Analyzer>, Option<String>, Option<String>, Option<String>)> {
    match args.backend.as_str() {
        "presidio" => Ok((
            Box::new(PresidioClient::new(&args.analyzer_url, args.timeout_secs)),
            None,
            None,
            None,
        )),
        "ollama" => {
            let model = args.ollama_model.clone()
                .ok_or_else(|| anyhow::anyhow!("--ollama-model is required when --backend ollama"))?;
            let prompt_path = args.system_prompt.clone()
                .unwrap_or_else(|| PathBuf::from("prompts/v1.md"));
            let prompt = std::fs::read_to_string(&prompt_path)
                .with_context(|| format!("system prompt not found: {}", prompt_path.display()))?;
            Ok((
                Box::new(OllamaClient::new(&args.analyzer_url, &model, &prompt, args.timeout_secs)),
                Some(model),
                Some(prompt_path.display().to_string()),
                Some(prompt),
            ))
        }
        other => anyhow::bail!("unknown backend {:?} — use 'presidio' or 'ollama'", other),
    }
}
```

Update the call site in `main()`:

```rust
let (analyzer, ollama_model, system_prompt_path, system_prompt_content) = build_analyzer(&args)?;
```

Update the `RunParams` construction in `main()`:

```rust
let params = RunParams {
    input:                  args.input.display().to_string(),
    analyzer_url:           args.analyzer_url,
    output:                 output_path.display().to_string(),
    recursive:              args.recursive,
    verbose:                args.verbose,
    backend:                args.backend,
    ollama_model,
    system_prompt_path,
    system_prompt_content,
    timeout_secs:           args.timeout_secs,
};
```

- [ ] **Step 6: Run full test suite**

```bash
cargo test
```
Expected: all tests PASS, no regressions.

- [ ] **Step 7: Commit**

```bash
git add src/model.rs src/main.rs
git commit -m "feat(report): include full system prompt content in JSON report"
```

---

## Task 2 — Create `prompts/` directory and move prompt file

**Files:**
- Create: `prompts/v1.md`

`llm-prompt.md` in the root is kept as-is for backward compatibility. The new default in `build_analyzer` (updated in Task 1) already points to `prompts/v1.md`.

- [ ] **Step 1: Copy `llm-prompt.md` to `prompts/v1.md`**

```bash
mkdir -p prompts
cp llm-prompt.md prompts/v1.md
```

- [ ] **Step 2: Verify the build and default path work**

```bash
cargo build 2>&1 | grep -E "^error" | head -5
```
Expected: no errors.

```bash
cargo test
```
Expected: all tests PASS.

- [ ] **Step 3: Commit**

```bash
git add prompts/v1.md
git commit -m "chore: add prompts/v1.md as first versioned system prompt"
```

---

## Verify (manual, after both tasks)

Build and run with Ollama to confirm the report now includes the prompt content:

```bash
cargo build --release

cargo run -- --backend ollama \
             --analyzer-url http://localhost:11434/api/chat \
             --ollama-model qwen2.5:7b-instruct-q4_K_M \
             --system-prompt prompts/v1.md \
             --input ./test-data

# Check the generated JSON report contains system_prompt_content
python3 -c "
import json, glob, sys
f = sorted(glob.glob('pii-eval-*.json'))[-1]
r = json.load(open(f))
content = r['params'].get('system_prompt_content', '')
print(f'prompt content length: {len(content)} chars')
print(f'first 80 chars: {content[:80]!r}')
"
```

Expected: prompt content length is > 0, first 80 chars match the beginning of `prompts/v1.md`.
