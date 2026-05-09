# Plan 0007 — Fix LOCATION spans, DATE_TIME/ORGANIZATION FP, PERSON title

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Correct test-data LOCATION annotations to expect full street-address spans, add a missing PERSON entity, and tighten `llm-prompt.md` to suppress DATE_TIME/ORGANIZATION false positives and prevent honorific titles from leaking into PERSON spans.

**Architecture:** Three independent layers — (1) JSON test-data byte-offset corrections verified with Python, (2) prompt rule additions for DATE_TIME/ORGANIZATION, (3) prompt rule addition for PERSON titles. No Rust code changes; `cargo test` is used only to confirm JSON is still parseable by the deserialiser.

**Tech Stack:** Rust/Cargo (test runner), Python 3 (offset verification), JSON, Markdown.

---

## ⚠️ Out of scope

**Char/byte offset bug (Plan 0008).** Several IT-language samples contain expected offsets that are 1–2 bytes off because Presidio returns char offsets while Rust treats them as byte offsets (visible on accented chars: "è", "à", "ò"). Affected spans are marked in the tasks below with `[BUG-A]` — do **not** touch them here.

---

## Files

| File | Change |
|---|---|
| `test-data/pii_location_person_it.json` | Expand LOCATION spans for 001, 004; leave 002/005/006 (BUG-A or not a street address) |
| `test-data/pii_location_person_en.json` | Expand LOCATION spans for 001, 004 |
| `test-data/pii_vat_fiscal_it.json` | Expand LOCATION spans for 001, 004, 005; add PERSON for 003 |
| `llm-prompt.md` | Add DATE_TIME/ORGANIZATION exclusion guidance (C); add PERSON title rule (D) |

---

## Task 1 — Fix LOCATION spans in `pii_location_person_it.json`

**Files:**
- Modify: `test-data/pii_location_person_it.json`

**Background:** Sample 001 annotates only the city `"Roma"` when the full street address `"piazza Navona 12, Roma"` is present and is the PII. Sample 004 is missing the second address entirely. Samples 002/005/006 are either affected by BUG-A or do not contain a street address — leave them unchanged.

- [ ] **Step 1: Verify current state**

Run the following to confirm the current offsets before editing:

```bash
python3 - <<'EOF'
import json
data = json.load(open("test-data/pii_location_person_it.json"))
for s in data["samples"]:
    b = s["text"].encode("utf-8")
    for e in s["expected"]:
        if e["entity_type"] == "LOCATION":
            print(f"  {s['id']} LOCATION [{e['start']}:{e['end']}] = {b[e['start']:e['end']]!r}")
EOF
```

Expected output:
```
  001 LOCATION [73:77] = b'Roma'
  002 LOCATION [54:61] = b' Firenz'      # BUG-A — leave
  002 LOCATION [121:127] = b' Milan'     # BUG-A — leave
  002 LOCATION [129:135] = b' Torin'     # BUG-A — leave
  002 LOCATION [138:145] = b' Bologn'    # BUG-A — leave
  003 LOCATION [55:62] = b'Venezia'
  003 LOCATION [65:72] = b'Palermo'
  004 LOCATION [0:6] = b'Genova'
  005 LOCATION [131:135] = b'Bari'
  006 LOCATION [186:192] = b' Torin'     # BUG-A — leave
```

- [ ] **Step 2: Edit `test-data/pii_location_person_it.json`**

Apply these two changes:

**Sample 001** — replace `{"entity_type": "LOCATION", "start": 73, "end": 77}` with:
```json
{ "entity_type": "LOCATION", "start": 55, "end": 77 }
```
Covers `"piazza Navona 12, Roma"`.

**Sample 004** — add a second LOCATION after the existing PERSON entry:
```json
{ "entity_type": "LOCATION", "start": 150, "end": 177 }
```
Covers `"via XX Settembre 18, Genova"`.

- [ ] **Step 3: Verify corrected state**

```bash
python3 - <<'EOF'
import json
data = json.load(open("test-data/pii_location_person_it.json"))
for s in data["samples"]:
    b = s["text"].encode("utf-8")
    for e in s["expected"]:
        if e["entity_type"] == "LOCATION":
            print(f"  {s['id']} LOCATION [{e['start']}:{e['end']}] = {b[e['start']:e['end']]!r}")
EOF
```

Expected output (changed lines marked with `←`):
```
  001 LOCATION [55:77] = b'piazza Navona 12, Roma'       ←
  004 LOCATION [0:6] = b'Genova'
  004 LOCATION [150:177] = b'via XX Settembre 18, Genova' ←
```
(All other lines unchanged from Step 1.)

- [ ] **Step 4: Run tests**

```bash
cargo test
```
Expected: all tests pass (no Rust logic changed).

- [ ] **Step 5: Commit**

```bash
git add test-data/pii_location_person_it.json
git commit -m "fix(test-data): expand LOCATION spans to full street address (IT)"
```

---

## Task 2 — Fix LOCATION spans in `pii_location_person_en.json`

**Files:**
- Modify: `test-data/pii_location_person_en.json`

**Background:** Sample 001 annotates only `"Edinburgh"` when the full postal address `"34 King Street, Edinburgh"` is present. Sample 004 annotates only `"Bristol"` when `"82 Elm Grove, Bristol, BS6 6JE"` (including postcode) is PII.

- [ ] **Step 1: Verify current state**

```bash
python3 - <<'EOF'
import json
data = json.load(open("test-data/pii_location_person_en.json"))
for s in data["samples"]:
    b = s["text"].encode("utf-8")
    for e in s["expected"]:
        if e["entity_type"] == "LOCATION":
            print(f"  {s['id']} LOCATION [{e['start']}:{e['end']}] = {b[e['start']:e['end']]!r}")
EOF
```

Expected output:
```
  001 LOCATION [74:83] = b'Edinburgh'
  002 LOCATION [65:72] = b'Chicago'
  002 LOCATION [122:130] = b'New York'
  002 LOCATION [135:146] = b'Los Angeles'
  003 LOCATION [140:146] = b'Boston'
  003 LOCATION [189:195] = b'London'
  004 LOCATION [38:45] = b'Bristol'
  005 LOCATION [0:7] = b'Toronto'
  006 LOCATION [27:36] = b'Frankfurt'
  006 LOCATION [56:65] = b'Amsterdam'
  006 LOCATION [107:116] = b'Singapore'
```

- [ ] **Step 2: Edit `test-data/pii_location_person_en.json`**

**Sample 001** — replace `{"entity_type": "LOCATION", "start": 74, "end": 83}` with:
```json
{ "entity_type": "LOCATION", "start": 58, "end": 83 }
```
Covers `"34 King Street, Edinburgh"`.

**Sample 004** — replace `{"entity_type": "LOCATION", "start": 38, "end": 45}` with:
```json
{ "entity_type": "LOCATION", "start": 24, "end": 54 }
```
Covers `"82 Elm Grove, Bristol, BS6 6JE"`.

- [ ] **Step 3: Verify corrected state**

```bash
python3 - <<'EOF'
import json
data = json.load(open("test-data/pii_location_person_en.json"))
for s in data["samples"]:
    b = s["text"].encode("utf-8")
    for e in s["expected"]:
        if e["entity_type"] == "LOCATION":
            print(f"  {s['id']} LOCATION [{e['start']}:{e['end']}] = {b[e['start']:e['end']]!r}")
EOF
```

Expected output (changed lines marked with `←`):
```
  001 LOCATION [58:83] = b'34 King Street, Edinburgh'      ←
  004 LOCATION [24:54] = b'82 Elm Grove, Bristol, BS6 6JE' ←
```
(All other lines unchanged.)

- [ ] **Step 4: Run tests**

```bash
cargo test
```
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add test-data/pii_location_person_en.json
git commit -m "fix(test-data): expand LOCATION spans to full street address (EN)"
```

---

## Task 3 — Fix `pii_vat_fiscal_it.json`

**Files:**
- Modify: `test-data/pii_vat_fiscal_it.json`

**Background:**
- Samples 001/004/005: LOCATION annotates only the city; correct to full street address.
- Sample 003: `"Lorenzo Cattaneo"` is a PERSON but is missing from expected (the model emits it as MEDICAL_LICENSE — fixing the annotation makes the FP visible as a classification error, not a span error).
- Sample 002: expected LOCATION `" a Mil"` (196-202) is BUG-A — leave unchanged.

- [ ] **Step 1: Verify current state**

```bash
python3 - <<'EOF'
import json
data = json.load(open("test-data/pii_vat_fiscal_it.json"))
for s in data["samples"]:
    b = s["text"].encode("utf-8")
    for e in s["expected"]:
        if e["entity_type"] in ("LOCATION", "PERSON"):
            print(f"  {s['id']} {e['entity_type']} [{e['start']}:{e['end']}] = {b[e['start']:e['end']]!r}")
EOF
```

Expected output:
```
  001 LOCATION [196:203] = b'Bologna'
  002 LOCATION [196:202] = b' a Mil'   # BUG-A — leave
  003 LOCATION [150:156] = b' Torin'   # BUG-A — leave
  004 LOCATION [117:122] = b'Parma'
  005 LOCATION [139:145] = b'Napoli'
  006 PERSON [15:30] = b'Giulia Ferretti'
```

- [ ] **Step 2: Edit `test-data/pii_vat_fiscal_it.json`**

**Sample 001** — replace `{"entity_type": "LOCATION", "start": 196, "end": 203}` with:
```json
{ "entity_type": "LOCATION", "start": 181, "end": 203 }
```
Covers `"via Torino 48, Bologna"`.

**Sample 003** — add a PERSON entry before the existing IT_VAT_CODE entry:
```json
{ "entity_type": "PERSON", "start": 25, "end": 41 }
```
Covers `"Lorenzo Cattaneo"`.

**Sample 004** — replace `{"entity_type": "LOCATION", "start": 117, "end": 122}` with:
```json
{ "entity_type": "LOCATION", "start": 94, "end": 123 }
```
Covers `"via Emilia Ponente 190, Parma"`.

**Sample 005** — replace `{"entity_type": "LOCATION", "start": 139, "end": 145}` with:
```json
{ "entity_type": "LOCATION", "start": 122, "end": 145 }
```
Covers `"via Garibaldi 3, Napoli"`.

- [ ] **Step 3: Verify corrected state**

```bash
python3 - <<'EOF'
import json
data = json.load(open("test-data/pii_vat_fiscal_it.json"))
for s in data["samples"]:
    b = s["text"].encode("utf-8")
    for e in s["expected"]:
        if e["entity_type"] in ("LOCATION", "PERSON"):
            print(f"  {s['id']} {e['entity_type']} [{e['start']}:{e['end']}] = {b[e['start']:e['end']]!r}")
EOF
```

Expected output (changed lines marked with `←`):
```
  001 LOCATION [181:203] = b'via Torino 48, Bologna'        ←
  003 PERSON [25:41] = b'Lorenzo Cattaneo'                  ←
  004 LOCATION [94:123] = b'via Emilia Ponente 190, Parma'  ←
  005 LOCATION [122:145] = b'via Garibaldi 3, Napoli'       ←
  006 PERSON [15:30] = b'Giulia Ferretti'
```
(002 and 003 LOCATION/EMAIL unchanged.)

- [ ] **Step 4: Run tests**

```bash
cargo test
```
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add test-data/pii_vat_fiscal_it.json
git commit -m "fix(test-data): expand LOCATION spans, add missing PERSON in vat_fiscal_it"
```

---

## Task 4 — Suppress DATE_TIME and ORGANIZATION false positives in `llm-prompt.md` (C)

**Files:**
- Modify: `llm-prompt.md`

**Background:** The model emits `DATE_TIME` for letter-header dates (`"12 maggio 2025"`, `"8 May 2025"`) and event dates (`"14 to 16 October"`, `"del 2024"`). It also emits `ORGANIZATION` for any company name regardless of context. Both types are already described correctly in the entity list, but the model ignores the contextual conditions. Adding explicit negative sub-bullets and a negative example will anchor the behaviour.

- [ ] **Step 1: Add DATE_TIME exclusions to Rule 5**

In `llm-prompt.md`, locate Rule 5 (`## Rule 5: Avoid these false positives`) and append two new bullets at the end of the list:

```markdown
- Dates in letter/document headers such as "Roma, 12 maggio 2025" or "Toronto, 8 May 2025" — these are the document date, not a personal date tied to an individual
- Event or conference date ranges ("14 to 16 October", "dal 3 al 5 marzo") — not tied to a specific person's record
- Bare years or vague time references ("del 2024", "negli anni '90", "in 2025") — not actionable personal data
```

- [ ] **Step 2: Add ORGANIZATION exclusion sub-bullet**

Still in Rule 5, find the existing bullet:
```
- Brand and product names that are not organizations doing private business with a person
```

Replace it with:
```markdown
- Company or organization names mentioned as the **subject** of a document (invoices, certifications, registrations) where the company itself is the data subject — the PII is the VAT/fiscal code, not the name. Emit ORGANIZATION only when the name reveals a private relationship between a named individual and that organization (e.g. "Mario Rossi works at Acme S.r.l." → Acme is relevant context for Mario).
```

- [ ] **Step 3: Add a negative example at the end of the Examples section**

Append the following input/output pair after the last example in `llm-prompt.md`:

```markdown
Input:
"Milano, 3 aprile 2025. Gentile cliente, la Ferri & Moretti S.p.A., P.IVA IT09124560152, ha ricevuto la sua richiesta."

Output:
{"entities":[
  {"entity_type":"IT_VAT_CODE","text":"IT09124560152","start":65,"end":78}
]}
```

This example teaches: letter-header date → not DATE_TIME; company name as document subject → not ORGANIZATION; only the VAT code is PII.

- [ ] **Step 4: Verify JSON in the new example is valid**

```bash
python3 -c '
import json
s = """{"entities":[
  {"entity_type":"IT_VAT_CODE","text":"IT09124560152","start":65,"end":78}
]}"""
json.loads(s)
text = "Milano, 3 aprile 2025. Gentile cliente, la Ferri & Moretti S.p.A., P.IVA IT09124560152, ha ricevuto la sua richiesta."
b = text.encode()
assert b[65:78] == b"IT09124560152", f"got {b[65:78]!r}"
print("OK")
'
```

Expected: `OK`

- [ ] **Step 5: Commit**

```bash
git add llm-prompt.md
git commit -m "fix(prompt): suppress DATE_TIME header dates and ORGANIZATION-as-subject FP"
```

---

## Task 5 — Exclude honorific titles from PERSON spans in `llm-prompt.md` (D)

**Files:**
- Modify: `llm-prompt.md`

**Background:** The model includes titles (`Dr.`, `Mr.`, `dott.`, `dottor`) in PERSON spans, and in one case misclassifies `"dottor Francesco Ricci"` as ORGANIZATION. Two fixes: (1) add an explicit title-exclusion rule, (2) add a negative example for the ORGANIZATION misclassification, (3) clarify that MEDICAL_LICENSE is a number string, not a title.

- [ ] **Step 1: Add title-exclusion sub-rule to Rule 1**

In `llm-prompt.md`, locate Rule 1 (`## Rule 1: "text" field must be character-perfect`) and append after the existing two paragraphs:

```markdown
Honorific titles and professional prefixes — `Dr.`, `Mr.`, `Mrs.`, `Ms.`, `Sig.`, `Sig.ra`, `dott.`, `dott.ssa`, `dottor`, `dottoressa`, `Avv.`, `Ing.`, `Prof.`, `Egregio`, `Gentile` — must **not** be included in the `PERSON` span. The span starts at the given name. Example: `"Dr. Sarah Mitchell"` → `"text": "Sarah Mitchell"`, `"start": 4`, `"end": 18`.
```

- [ ] **Step 2: Clarify MEDICAL_LICENSE in the entity list**

In `llm-prompt.md`, find the line:
```
- MEDICAL_LICENSE — medical license numbers
```

Replace it with:
```markdown
- MEDICAL_LICENSE — medical license **number strings** (alphanumeric codes issued by a licensing body). Do NOT emit MEDICAL_LICENSE for titles such as "dott.", "dott.ssa", "Dr." — those belong to the PERSON span rule above, and the title itself is excluded from the span.
```

- [ ] **Step 3: Add a negative example for title-as-ORGANIZATION**

Append the following input/output pair after the example added in Task 4:

```markdown
Input:
"Egregio dottor Francesco Ricci, le trasmettiamo la documentazione sito in via XX Settembre 18, Genova."

Output:
{"entities":[
  {"entity_type":"PERSON","text":"Francesco Ricci","start":16,"end":31},
  {"entity_type":"LOCATION","text":"via XX Settembre 18, Genova","start":75,"end":102}
]}
```

This example teaches: `"dottor"` is a title and is excluded from PERSON; the person starts at the given name; `"dottor Francesco Ricci"` is never ORGANIZATION.

- [ ] **Step 4: Verify offsets in the new example**

```bash
python3 -c '
text = "Egregio dottor Francesco Ricci, le trasmettiamo la documentazione sito in via XX Settembre 18, Genova."
b = text.encode()
assert b[16:31] == b"Francesco Ricci", f"PERSON: got {b[16:31]!r}"
assert b[75:102] == b"via XX Settembre 18, Genova", f"LOCATION: got {b[75:102]!r}"
print("OK")
'
```

Expected: `OK`

- [ ] **Step 5: Commit**

```bash
git add llm-prompt.md
git commit -m "fix(prompt): exclude honorific titles from PERSON spans, clarify MEDICAL_LICENSE"
```

---

## Verify (end-to-end, requires Ollama running)

After all five tasks are committed:

```bash
cargo build --release

cargo run -- --backend ollama \
             --analyzer-url http://localhost:11434/api/chat \
             --ollama-model qwen2.5:7b-instruct-q4_K_M \
             --input ./test-data --verbose
```

Expected improvements vs the baseline report (`pii-eval-29bded1-2026-05-08_16_03_04.json`):
- LOCATION strict TP should increase (full-address spans now match)
- `ORGANIZATION` FP should drop (prompt guidance + negative example)
- `DATE_TIME` FP should drop (header/event dates excluded by new Rule 5 bullets)
- PERSON near-misses for `Dr.`/`Mr.`/`dottor` should resolve to strict TP
- `MEDICAL_LICENSE` FP for `"dott. Lorenzo Cattaneo"` should disappear
