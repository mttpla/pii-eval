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
| [2026-05-09](../superpowers/plans/2026-05-09-unicode-char-offsets.md) | Unicode char offsets — src/unicode.rs helpers, fix checker.rs + ollama_client.rs to use char offsets consistently |
| [2026-05-09](../superpowers/plans/2026-05-09-prompt-content-in-report.md) | Prompt versioning — prompts/v1.md, system_prompt_content in JSON report, default path updated |

## Backlog

| Priority | Title | Description |
|---|---|---|
| medium | Report comparison tool | Add a CLI subcommand or script to diff two report JSONs side-by-side (delta per entity type and language). pii-eval is a benchmark — prompts, models, and backends are the variables; test data is the constant. Each report already embeds system_prompt_content, so two reports are self-contained and comparable. |
| low | Ollama model-not-available error | When the requested model is not loaded, surface a clear error and suggest running `ollama pull <model-name>` instead of a generic api_error. |
