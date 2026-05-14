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
| [2026-05-10](../superpowers/plans/2026-05-10-ollama-warmup.md) | Ollama warm-up — `--warmup-timeout-secs` flag (default 300), `OllamaClient::warmup()`, `warmup_timeout_secs` in `RunParams` |
| [2026-05-10](../superpowers/plans/2026-05-10-ollama-model-pull.md) | Ollama auto-pull — `ensure_ready()`, `pull()`, `--pull-timeout-secs` (default 600s), exclude pull time from `elapsed`, `pull_elapsed` in report |

## Backlog

| Priority | Title | Description |
|---|---|---|
| medium | Report comparison tool | Add a CLI subcommand or script to diff two report JSONs side-by-side (delta per entity type and language). pii-eval is a benchmark — prompts, models, and backends are the variables; test data is the constant. Each report already embeds system_prompt_content, so two reports are self-contained and comparable. |
| low | Warn when --system-prompt ignored | When `--backend presidio` and `--system-prompt` is passed, the flag is silently ignored (file not read, no warning). One `eprintln!` warning in `build_analyzer` is enough. |
| medium | OpenAI Privacy Filter backend | New `openai_privacy_client.rs` implementing `Analyzer` trait. Cloud API — POST text, get PII spans back. Add `--backend openai-privacy` CLI flag, `--openai-api-key` arg (or `OPENAI_API_KEY` env var), record `backend = "openai-privacy"` in report. Offset format TBD from API docs (https://openai.com/it-IT/index/introducing-openai-privacy-filter/). |
| medium | Azure AI Language PII backend | New `azure_language_client.rs` implementing `Analyzer` trait. REST API `analyze-text` action `PiiEntityRecognition`. Good multilingual coverage including Italian. Add `--backend azure-language`, `--azure-endpoint`, `--azure-api-key` args. Map Azure entity categories to Presidio entity_type names for cross-backend comparison. |
| medium | AWS Comprehend PII backend | New `aws_comprehend_client.rs` implementing `Analyzer` trait. `DetectPiiEntities` API, useful for English-language benchmark baseline. Add `--backend aws-comprehend`, `--aws-region`, AWS credentials via env vars. Map AWS entity types to Presidio names. |
| low | PasteGuard backend | Local CLI-based anonymizer (https://github.com/sgasser/pasteguard). Spawn as subprocess or wrap HTTP if it exposes one. Add `--backend pasteguard` + `--pasteguard-bin` path arg. Useful for fully offline eval runs without Ollama overhead. |
| low | Generic HTTP backend | Config file (JSON/TOML) mapping endpoint URL + request/response field schema. Allows plugging in any future analyzer without code changes — define `input_field`, `output_array`, `entity_type_field`, `start_field`, `end_field`. High leverage for one-off experiments. |
