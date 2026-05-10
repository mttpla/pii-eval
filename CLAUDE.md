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

New plans live in `docs/superpowers/plans/YYYY-MM-DD-<feature-name>.md`.  
**Always use the `superpowers:writing-plans` skill** to write a new plan and `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to execute it.

> Legacy plans 0001–0007 in `docs/plans/NNNN_topic.md` are completed and left as-is.

### Index

`docs/plans/INDEX.md` tracks history and future ideas:
- **Completed** — storico dei piani terminati e mergiati.
- **Backlog** — appunti su lavoro futuro (nessun file piano ancora). Quando un'idea diventa concreta, esegui `superpowers:writing-plans` e crea il file.

Active plans are tracked by their own checkboxes inside the plan file — INDEX.md does not duplicate that state.

## Periodic practices

At the end of a significant task or when the user runs `git push`, remind them (once, briefly) to consider:

1. **Code review with Opus** — run `/ultrareview` or spawn an Opus agent to review the diff/branch for issues, smells, and improvements.
2. **Test gap analysis** — look at the changed modules and ask: are there untested paths, edge cases, or behaviours worth covering? Suggest specific additions, not generic "add more tests".

Do NOT remind on trivial changes (doc-only edits, comment fixes, backlog updates). Only when real code changed.
