# Plan Index

## Completed

| Plan | Title |
|---|---|
| [0001](0001_initial_build.md) | pii-eval — Initial build |
| [0002](0002_output_params.md) | Auto-generated output filename + params in report |
| [0003](0003_span_text.md) | Extracted text in error output |
| [0004](0004_na_zero_metrics.md) | N/A display for zero-count metric rows |
| [0005](0005_elapsed_time.md) | Total execution time in report and stdout |
| [0006](0006_ollama_backend.md) | Ollama LLM backend — trait Analyzer, OllamaClient, Option C offset resolution, all CLI params in report |

## Backlog

| Priority | Title | Description |
|---|---|---|
| high | Fix char/byte offset mismatch | Presidio (Python) returns character offsets; Rust treats them as byte offsets. Invisible on ASCII, breaks on accented Italian text (e.g. "è", "à"). Must be fixed before adding non-ASCII test data. |
| low | Ollama model-not-available error | When the requested model is not loaded, surface a clear error and suggest running `ollama pull <model-name>` instead of a generic api_error. |
