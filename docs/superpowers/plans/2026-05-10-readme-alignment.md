# README Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align README.md with the current state of the codebase — fix the offset description, the JSON report example, and add a System Prompt section.

**Architecture:** Three independent README edits, each committed separately. No code changes — code correctness is already verified by tests. These are documentation corrections only.

**Tech Stack:** Markdown.

---

## Background

Three things in README.md are currently wrong or missing:

1. **Offset description** says "byte positions" — since the unicode fix, pii-eval uses Unicode character offsets throughout. The old "Tip" about `text.encode('utf-8')` is now actively misleading.

2. **JSON report example** is missing the `params` block entirely (which includes `system_prompt_path` and `system_prompt_content`), `elapsed` is missing from `summary`, and the `test_errors` structure uses wrong field names (`expected`/`obtained` don't exist — the actual fields are `false_negatives`, `false_positives`, `near_misses`, each containing `SpanWithText` objects with a `text` field).

3. **No section about prompts** — there is no explanation of `prompts/`, prompt versioning, or how the full prompt text is recorded in every report.

---

## Files

| File | Change |
|---|---|
| `README.md` | Three edits: offset description, JSON example, new System Prompt section |

---

## Task 1 — Fix offset description and remove misleading UTF-8 tip

**Files:**
- Modify: `README.md` (section "Step 3 — Get the offsets right", lines ~164–183)

### Current text to replace

Line ~164:
```
Offsets are **byte positions** in the UTF-8 string: `start` is inclusive, `end` is exclusive. The easiest way to verify them is Python:
```

Lines ~182–183 (the misleading tip at the bottom of Step 3):
```
> **Tip**: if the text contains non-ASCII characters (accented letters, emoji), remember that Python `str` uses Unicode code points while Presidio works on UTF-8 bytes. For pure Latin text they coincide; for anything else, use `text.encode('utf-8')` to find byte offsets.
```

### What to put in their place

Line ~164 replacement:
```
Offsets are **Unicode character offsets**: `start` is inclusive, `end` is exclusive. The easiest way to verify them is Python:
```

Tip replacement:
```
> **Tip**: Python `str` indexing uses Unicode code points — the same convention used by pii-eval, Presidio, and the Ollama prompt. `text[start:end]` always gives the expected substring, even for accented Italian text (`è`, `à`, `ù`) or emoji.
```

- [ ] **Step 1: Make the two edits in README.md**

Find and replace exactly:

**Edit A** — change the first sentence of Step 3:
Old: `Offsets are **byte positions** in the UTF-8 string: `start` is inclusive, `end` is exclusive.`
New: `Offsets are **Unicode character offsets**: `start` is inclusive, `end` is exclusive.`

**Edit B** — replace the Tip blockquote at the bottom of Step 3:
Old:
```
> **Tip**: if the text contains non-ASCII characters (accented letters, emoji), remember that Python `str` uses Unicode code points while Presidio works on UTF-8 bytes. For pure Latin text they coincide; for anything else, use `text.encode('utf-8')` to find byte offsets.
```
New:
```
> **Tip**: Python `str` indexing uses Unicode code points — the same convention used by pii-eval, Presidio, and the Ollama prompt. `text[start:end]` always gives the expected substring, even for accented Italian text (`è`, `à`, `ù`) or emoji.
```

- [ ] **Step 2: Verify**

Read the "Step 3 — Get the offsets right" section in README.md and confirm:
- No mention of "byte positions" or "UTF-8 bytes" in the offset description
- The Tip talks about Unicode code points being the shared convention

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(readme): fix offset description — char offsets, not byte positions"
```

---

## Task 2 — Fix the JSON report example

**Files:**
- Modify: `README.md` (section "JSON report structure", lines ~333–369)

### What is wrong

The current example:
- Has no `params` block (it was added and is now the third top-level field in `EvalReport`)
- Has no `elapsed` in `summary` (added in an earlier plan)
- Has wrong `test_errors` structure: uses `expected`/`obtained` arrays which do not exist. The real fields are `near_misses` (array of `{obtained, expected}` pairs), `false_positives` (array), `false_negatives` (array). Each item is a `SpanWithText`: `{entity_type, start, end, text}`.
- The Ollama-specific optional fields (`ollama_model`, `system_prompt_path`, `system_prompt_content`) don't appear anywhere

### Correct JSON report structure (Ollama backend run)

The `EvalReport` struct serializes in this field order:
`version` → `generated_at` → `params` → `summary` → `global` → `by_entity_type` → `by_language` → `test_errors` → `api_errors`

The correct `RunParams` for Ollama (optional fields present only when set):
```json
"params": {
  "input": "./test-data",
  "analyzer_url": "http://localhost:11434/api/chat",
  "output": "pii-eval-a1b2c3d-2026-05-07_10_00_00.json",
  "recursive": false,
  "verbose": false,
  "backend": "ollama",
  "ollama_model": "qwen2.5:7b-instruct-q4_K_M",
  "system_prompt_path": "prompts/v1.md",
  "system_prompt_content": "You are a PII detection engine like the Microsoft Presidio Analyzer...",
  "timeout_secs": 120
}
```

For Presidio, `ollama_model`, `system_prompt_path`, `system_prompt_content` are all absent (not null — they are skipped entirely by `skip_serializing_if = "Option::is_none"`).

The correct `summary`:
```json
"summary": {
  "files": 3,
  "samples": 42,
  "api_errors": 0,
  "entities_expected": 130,
  "entities_predicted": 127,
  "elapsed": "00:01:23"
}
```

The correct `test_errors` entry (false negative example):
```json
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
  }
]
```

A near miss entry looks like:
```json
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
```

- [ ] **Step 1: Replace the entire JSON report section in README.md**

Find the heading `### JSON report structure` and replace **everything from that heading to the closing `---` separator** (i.e. the heading, the single fenced code block, and the `---`) with the corrected version below:

````markdown
### JSON report structure

The report shows two variants — Presidio (rule-based, no prompt) and Ollama (LLM with system prompt) — since the `params` block differs between them.

**Presidio backend:**

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
    "entities_predicted": 127,
    "elapsed": "00:00:08"
  },
  "global": {
    "strict":  { "tp": 115, "fp": 10, "fn": 15, "precision": 0.921, "recall": 0.884, "f1": 0.902, "f2": 0.891 },
    "relaxed": { "tp": 118, "fp":  7, "fn": 12, "precision": 0.945, "recall": 0.907, "f1": 0.926, "f2": 0.914 }
  },
  "by_entity_type": {
    "PERSON": { "strict": { "tp": 50, "fp": 2, "fn": 3, "precision": 0.962, "recall": 0.943, "f1": 0.952, "f2": 0.947 },
                "relaxed": { "..." } }
  },
  "by_language": {
    "it": { "strict": { "..." }, "relaxed": { "..." } }
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

**Ollama backend** — same structure, with additional prompt fields in `params` and typically longer `elapsed`:

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
    "system_prompt_content": "You are a PII detection engine like the Microsoft Presidio Analyzer...",
    "timeout_secs": 120
  },
  "summary": {
    "files": 3,
    "samples": 42,
    "api_errors": 0,
    "entities_expected": 130,
    "entities_predicted": 127,
    "elapsed": "00:12:45"
  },
  "global": { "..." },
  "by_entity_type": { "..." },
  "by_language": { "..." },
  "test_errors": [ "..." ],
  "api_errors": []
}
```
````

- [ ] **Step 2: Verify**

Read the "JSON report structure" section and confirm:
- Two variants shown (Presidio and Ollama)
- `params` block present in both, with Ollama including `ollama_model`, `system_prompt_path`, `system_prompt_content`
- `summary` includes `elapsed`
- `test_errors` uses `near_misses`, `false_positives`, `false_negatives` — each item has `entity_type`, `start`, `end`, `text`
- Near miss shows `obtained`/`expected` pair structure

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(readme): fix JSON report example — add params, elapsed, correct test_errors structure"
```

---

## Task 3 — Add System Prompt section

**Files:**
- Modify: `README.md` (add new section after "Running Ollama locally")

### What to add

A new top-level section `## System prompt` explaining:
- Prompt files live in `prompts/`, current default is `prompts/v1.md`
- How to create a new prompt variant for experimentation
- The full prompt text is saved in the JSON report under `params.system_prompt_content`
- How this enables comparing two runs

- [ ] **Step 1: Add the section after the "Running Ollama locally" section**

Find the line `---` that follows the Ollama section (the one before `## Running Presidio locally`) and insert the new section before it:

```markdown
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
pii-eval --system-prompt prompts/v1.md --backend ollama ... --output report-v1.json
pii-eval --system-prompt prompts/v2.md --backend ollama ... --output report-v2.json
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
```

- [ ] **Step 2: Verify**

Read the new section and confirm:
- Explains `prompts/` directory and `prompts/v1.md` as default
- Shows how to run with a specific prompt (`--system-prompt`)
- Explains how to create a new variant and compare two runs
- States that `system_prompt_content` is recorded in the report
- Notes that the field is absent for Presidio

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(readme): add System Prompt section — prompts/ directory, versioning, report recording"
```
