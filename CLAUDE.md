# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build --release          # build
cargo test                     # run all tests
cargo test checker             # run tests for a single module (e.g. checker)
cargo test extracted_text      # run a single test by name
cargo run                      # run with all defaults
cargo run -- --input ./test-data --analyzer-url http://localhost:5002/analyze --verbose
```

The version string embedded in the binary and every report is the git SHA, injected by `build.rs` via `cargo:rustc-env=GIT_SHA`. Rebuild after committing to see the new SHA.

## Architecture

Seven modules with strict SRP — no module does more than one thing:

```
model.rs            all shared types, no logic
presidio_client.rs  HTTP only — POST /analyze, return Vec<PredictedSpan> or error
checker.rs          pure matching logic — no I/O, no HTTP
stats.rs            stateful accumulator — add() per sample, finalize() at end
report.rs           print_console() + write_json() — presentation only
main.rs             CLI args + per-file orchestration loop
build.rs            injects GIT_SHA env var at compile time
```

### Data flow

```
test-data/*.json  →  main (one file at a time, dropped after processing)
                  →  presidio_client.analyze(text, lang) → Vec<PredictedSpan>
                  →  checker.check(id, source_text, predicted, expected)
                         → CheckResult { counts, by_type: BTreeMap, error: Option<TestError> }
                  →  stats.add(counts, by_type, lang)
                  →  [end] stats.finalize() → (DualMetrics, by_entity_type, by_language)
                  →  report.print_console + report.write_json
```

### Key design decisions

- `ExpectedEntity` has **no `text` field** — match is on `entity_type + start + end` only. Text is extracted from `source_text` at error-build time and stored in `SpanWithText` (used only in `TestError`).
- `PredictedSpan` and `ExpectedEntity` are pure offset types; `SpanWithText` is the presentation type used in errors.
- **Strict match**: same type + same start + same end. **Relaxed match**: same type + spans overlap ≥ 1 char.
- `ErrorSeverity` priority: `High` (any FalseNegative) > `Medium` (NearMiss only) > `Low` (FalsePositive only).
- `checker.rs` returns `CheckResult.by_type: BTreeMap<String, SampleCounts>` — per-entity-type counts computed directly from the match arrays, not re-aggregated post-hoc.
- Output filename auto-generates as `pii-eval-{SHA}-{YYYY-MM-DD_HH_mm_SS}.json` when `--output` is omitted.

## Test data format

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

Offsets are byte positions (exclusive end). Verify with:
```python
text[start:end]  # should match the intended entity text exactly
```

## Plans

Implementation plans live in `docs/plans/`. Naming convention: `NNNN_topic.md` where `NNNN` is the next zero-padded 4-digit number and `topic` is snake_case.

**Always use the `superpowers:writing-plans` skill** to write a new plan — it produces the standard header, bite-sized TDD tasks with checkboxes (`- [ ]`), and a self-review pass.

**To execute a plan**, use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`.

### Mandatory plan header

```markdown
# [Feature Name] Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** [one sentence]
**Architecture:** [2-3 sentences]
**Tech Stack:** Rust, Cargo, anyhow, serde, reqwest
```

### Task structure (bite-sized, TDD)

Each task = one file/component. Steps are 2-5 min each:

```markdown
### Task N: [Name]

**Files:**
- Create/Modify: `src/file.rs`
- Test: inline unit tests in same file

- [ ] Write the failing test (actual test code here)
- [ ] Run `cargo test <name>` — expected: FAIL
- [ ] Write minimal implementation (actual code here)
- [ ] Run `cargo test <name>` — expected: PASS
- [ ] `git commit -m "feat: ..."`
```

No placeholders ("TBD", "add appropriate error handling", etc.) — every step shows real code and expected command output.

### Index

Always create a new file — never edit a completed plan.

`docs/plans/INDEX.md` is the authoritative backlog. Keep it up to date:
- Move items from **Backlog** to a numbered plan file when work begins.
- Mark a plan **Completed** when all checkboxes are done and merged.
- Progress within a plan is tracked by its own checkboxes — INDEX.md does not track "In Progress".
