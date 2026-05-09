# Unicode Char Offsets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make pii-eval use Unicode character offsets consistently throughout so that Presidio, the LLM, hand-written test data, and Rust all speak the same language.

**Architecture:** Add a `src/unicode.rs` module with two helpers (`char_to_byte`, `byte_to_char`), then fix the two call sites that currently do byte slicing: `checker.rs::span_with_text` and `ollama_client.rs::resolve_offset` + `find_next_occurrence`. No changes to test data — the BUG-A samples were already annotated with Python char offsets and will automatically become correct once Rust interprets them the same way.

**Tech Stack:** Rust, Cargo (no new dependencies).

---

## Background: why this matters

Rust `&str` indexing is byte-based (`text[10..20]` = bytes 10 to 20). Python string indexing is char-based (`text[10:20]` = chars 10 to 20). For ASCII they coincide; for UTF-8 multibyte chars (Italian "è", "à", "ù" = 2 bytes each) they diverge.

Every producer of offsets in this project uses char offsets:
- Presidio returns char offsets (Python)
- The LLM is told to emit char offsets (`"Unicode character offsets, consistent with Python text[start:end]"`)
- Test data is written and verified with Python `text[start:end]`

Only the Rust consumer was treating them as byte offsets. This plan fixes Rust.

---

## Files

| File | Change |
|---|---|
| `src/unicode.rs` | New: `char_to_byte(text, char_offset) → usize` and `byte_to_char(text, byte_offset) → usize` |
| `src/main.rs` | Add `mod unicode;` |
| `src/checker.rs` | Fix `span_with_text` to use `char_to_byte` for text extraction |
| `src/ollama_client.rs` | Fix `resolve_offset` (validation) and `find_next_occurrence` (fallback) to use char offsets |

---

## Task 1 — `src/unicode.rs`: Unicode offset helpers

**Files:**
- Create: `src/unicode.rs`
- Modify: `src/main.rs` (add `mod unicode;`)

### What we need

```rust
/// Converts a Unicode char offset to a UTF-8 byte offset.
/// Returns `text.len()` if `char_offset` is past the end.
pub fn char_to_byte(text: &str, char_offset: usize) -> usize {
    text.char_indices()
        .nth(char_offset)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

/// Converts a UTF-8 byte offset to a Unicode char offset.
/// Panics if `byte_offset` is not on a char boundary (same as standard Rust slice).
pub fn byte_to_char(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset].chars().count()
}
```

- [ ] **Step 1: Write the failing tests**

Create `src/unicode.rs` containing only the tests (no implementation yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_to_byte_ascii_identity() {
        assert_eq!(char_to_byte("hello", 0), 0);
        assert_eq!(char_to_byte("hello", 3), 3);
        assert_eq!(char_to_byte("hello", 5), 5);
    }

    #[test]
    fn char_to_byte_multibyte() {
        // "cafè": c(0) a(1) f(2) è(3) — è is 2 bytes (0xC3 0xA8)
        // byte layout: c=0 a=1 f=2 è=3,4  → len=5
        assert_eq!(char_to_byte("cafè", 0), 0); // 'c'
        assert_eq!(char_to_byte("cafè", 3), 3); // 'è' starts at byte 3
        assert_eq!(char_to_byte("cafè", 4), 5); // past end → text.len()
    }

    #[test]
    fn char_to_byte_italian_sentence() {
        // "può" = p(0) u(1) ò(2) — ò is 2 bytes
        // byte layout: p=0 u=1 ò=2,3 → len=4
        assert_eq!(char_to_byte("può", 2), 2); // 'ò' starts at byte 2
        assert_eq!(char_to_byte("può", 3), 4); // past end → text.len()
    }

    #[test]
    fn char_to_byte_past_end_returns_len() {
        assert_eq!(char_to_byte("hello", 99), 5);
        assert_eq!(char_to_byte("", 0), 0);
    }

    #[test]
    fn byte_to_char_ascii_identity() {
        assert_eq!(byte_to_char("hello", 0), 0);
        assert_eq!(byte_to_char("hello", 3), 3);
        assert_eq!(byte_to_char("hello", 5), 5);
    }

    #[test]
    fn byte_to_char_multibyte() {
        // "cafè": byte 5 (past end) = char 4
        assert_eq!(byte_to_char("cafè", 0), 0);
        assert_eq!(byte_to_char("cafè", 3), 3); // byte 3 = start of 'è' = char 3
        assert_eq!(byte_to_char("cafè", 5), 4); // byte 5 = past end = 4 chars
    }

    #[test]
    fn roundtrip_char_to_byte_to_char() {
        let text = "Mi chiamo Léa è qui";
        for char_offset in 0..=text.chars().count() {
            let byte_offset = char_to_byte(text, char_offset);
            assert_eq!(byte_to_char(text, byte_offset), char_offset);
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they FAIL**

```bash
cargo test unicode
```
Expected: FAIL with `cannot find function char_to_byte` or similar.

- [ ] **Step 3: Implement the two helpers**

Replace the contents of `src/unicode.rs` with the full implementation + tests:

```rust
/// Converts a Unicode char offset to a UTF-8 byte offset.
/// Returns `text.len()` if `char_offset` is past the end.
pub fn char_to_byte(text: &str, char_offset: usize) -> usize {
    text.char_indices()
        .nth(char_offset)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

/// Converts a UTF-8 byte offset to a Unicode char offset.
pub fn byte_to_char(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset].chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_to_byte_ascii_identity() {
        assert_eq!(char_to_byte("hello", 0), 0);
        assert_eq!(char_to_byte("hello", 3), 3);
        assert_eq!(char_to_byte("hello", 5), 5);
    }

    #[test]
    fn char_to_byte_multibyte() {
        assert_eq!(char_to_byte("cafè", 0), 0);
        assert_eq!(char_to_byte("cafè", 3), 3);
        assert_eq!(char_to_byte("cafè", 4), 5);
    }

    #[test]
    fn char_to_byte_italian_sentence() {
        assert_eq!(char_to_byte("può", 2), 2);
        assert_eq!(char_to_byte("può", 3), 4);
    }

    #[test]
    fn char_to_byte_past_end_returns_len() {
        assert_eq!(char_to_byte("hello", 99), 5);
        assert_eq!(char_to_byte("", 0), 0);
    }

    #[test]
    fn byte_to_char_ascii_identity() {
        assert_eq!(byte_to_char("hello", 0), 0);
        assert_eq!(byte_to_char("hello", 3), 3);
        assert_eq!(byte_to_char("hello", 5), 5);
    }

    #[test]
    fn byte_to_char_multibyte() {
        assert_eq!(byte_to_char("cafè", 0), 0);
        assert_eq!(byte_to_char("cafè", 3), 3);
        assert_eq!(byte_to_char("cafè", 5), 4);
    }

    #[test]
    fn roundtrip_char_to_byte_to_char() {
        let text = "Mi chiamo Léa è qui";
        for char_offset in 0..=text.chars().count() {
            let byte_offset = char_to_byte(text, char_offset);
            assert_eq!(byte_to_char(text, byte_offset), char_offset);
        }
    }
}
```

- [ ] **Step 4: Wire up the module in `src/main.rs`**

Add `mod unicode;` near the top of `src/main.rs`, alongside the other `mod` declarations.

- [ ] **Step 5: Run tests to verify they PASS**

```bash
cargo test unicode
```
Expected: all unicode tests PASS, full suite still PASS.

- [ ] **Step 6: Commit**

```bash
git add src/unicode.rs src/main.rs
git commit -m "feat: add unicode char_to_byte / byte_to_char helpers"
```

---

## Task 2 — `src/checker.rs`: char-based text extraction in `span_with_text`

**Files:**
- Modify: `src/checker.rs`

### Context

`span_with_text` (line 193) currently does `source.get(start..end)` which is a **byte** slice. All offsets in this project are char offsets, so this must use `char_to_byte` to convert before slicing.

Current code:
```rust
fn span_with_text(entity_type: &str, start: usize, end: usize, source: &str) -> SpanWithText {
    SpanWithText {
        entity_type: entity_type.to_string(),
        start,
        end,
        text: source.get(start..end).unwrap_or("").to_string(),
    }
}
```

- [ ] **Step 1: Write the failing test**

Add this test to the `#[cfg(test)]` block in `src/checker.rs`:

```rust
#[test]
fn extracted_text_multibyte_chars() {
    // "Mi chiamo Léa Dubois"
    // char layout: M(0)i(1) (2)c(3)h(4)i(5)a(6)m(7)o(8) (9)L(10)é(11)a(12) (13)D(14)u(15)b(16)o(17)i(18)s(19)
    // "Léa" = chars 10..13, bytes 10..14 (é is 2 bytes)
    // "Dubois" = chars 14..20, bytes 15..21
    let source = "Mi chiamo Léa Dubois";

    // FP: predicted "Léa" at char 10..13
    let r = check("t::1", source, &[pred("PERSON", 10, 13)], &[]);
    let err = r.error.unwrap();
    assert_eq!(err.false_positives[0].text, "Léa");

    // FP: predicted "Dubois" at char 14..20
    let r2 = check("t::1", source, &[pred("LOCATION", 14, 20)], &[]);
    let err2 = r2.error.unwrap();
    assert_eq!(err2.false_positives[0].text, "Dubois");

    // NearMiss: predicted chars 10..20 "Léa Dubois", expected chars 14..20 "Dubois"
    let r3 = check(
        "t::1", source,
        &[pred("PERSON", 10, 20)],
        &[exp("PERSON", 14, 20)],
    );
    let err3 = r3.error.unwrap();
    assert_eq!(err3.near_misses[0].obtained.text, "Léa Dubois");
    assert_eq!(err3.near_misses[0].expected.text, "Dubois");
}
```

- [ ] **Step 2: Run test to verify it FAILS**

```bash
cargo test extracted_text_multibyte_chars
```
Expected: FAIL — `err.false_positives[0].text` will be a garbled byte slice instead of `"Léa"`.

- [ ] **Step 3: Fix `span_with_text`**

Replace the function in `src/checker.rs`:

```rust
fn span_with_text(entity_type: &str, start: usize, end: usize, source: &str) -> SpanWithText {
    let start_byte = crate::unicode::char_to_byte(source, start);
    let end_byte   = crate::unicode::char_to_byte(source, end);
    SpanWithText {
        entity_type: entity_type.to_string(),
        start,
        end,
        text: source.get(start_byte..end_byte).unwrap_or("").to_string(),
    }
}
```

- [ ] **Step 4: Run tests to verify they PASS**

```bash
cargo test checker
```
Expected: all checker tests PASS including the new `extracted_text_multibyte_chars`.

- [ ] **Step 5: Commit**

```bash
git add src/checker.rs
git commit -m "fix(checker): use char offsets for text extraction in span_with_text"
```

---

## Task 3 — `src/ollama_client.rs`: char offsets in offset resolution

**Files:**
- Modify: `src/ollama_client.rs`

### Context

Two functions need fixing:

**`resolve_offset` (line 167):** Validates the LLM-provided offsets by checking `source.get(entity.start..entity.end)`. This is a byte slice but the LLM emits char offsets.

**`find_next_occurrence` (line 177):** Uses `match_indices` which returns **byte** offsets. The results are stored in `used` and returned as `PredictedSpan` offsets. Must convert to char offsets.

After this fix the `used` vector and all returned spans will carry char offsets, which is what `checker.rs` and the test data expect.

Current `resolve_offset`:
```rust
fn resolve_offset(source: &str, entity: &LlmEntity, used: &[(usize, usize)]) -> Option<(usize, usize)> {
    if entity.text.is_empty() {
        return None;
    }
    let llm_range = (entity.start, entity.end);
    if source.get(entity.start..entity.end) == Some(entity.text.as_str())
        && !overlaps_any(llm_range, used)
    {
        return Some(llm_range);
    }
    find_next_occurrence(source, &entity.text, used)
}
```

Current `find_next_occurrence`:
```rust
fn find_next_occurrence(source: &str, needle: &str, used: &[(usize, usize)]) -> Option<(usize, usize)> {
    source
        .match_indices(needle)
        .find(|(start, s)| !overlaps_any((*start, start + s.len()), used))
        .map(|(start, s)| (start, start + s.len()))
}
```

- [ ] **Step 1: Write failing tests**

Add these tests to the `#[cfg(test)]` block in `src/ollama_client.rs`:

```rust
#[test]
fn resolve_offset_multibyte_valid_char_offsets() {
    // "Mi chiamo Léa Dubois"
    // "Léa" = chars 10..13
    // LLM emits char offsets 10..13 and text "Léa"
    let src = "Mi chiamo Léa Dubois";
    let e = entity("PERSON", "Léa", 10, 13);
    assert_eq!(resolve_offset(src, &e, &[]), Some((10, 13)));
}

#[test]
fn resolve_offset_multibyte_invalid_offsets_fallback() {
    // LLM emits wrong offsets but correct text → fallback to text search
    // Must return char offsets, not byte offsets
    let src = "Mi chiamo Léa Dubois";
    let e = entity("PERSON", "Léa", 999, 1002); // garbage offsets
    assert_eq!(resolve_offset(src, &e, &[]), Some((10, 13))); // char 10..13
}

#[test]
fn find_next_occurrence_multibyte_returns_char_offsets() {
    // "Léa" = chars 10..13 (bytes 10..14)
    // Must return char offsets (10, 13), NOT byte offsets (10, 14)
    let src = "Mi chiamo Léa Dubois";
    assert_eq!(find_next_occurrence(src, "Léa", &[]), Some((10, 13)));
}

#[test]
fn find_next_occurrence_multibyte_skips_used_char_offsets() {
    let src = "Léa e Léa";
    // "Léa" appears at char 0..3 and char 6..9
    // With (0, 3) in used (char offsets), must return (6, 9)
    assert_eq!(find_next_occurrence(src, "Léa", &[(0, 3)]), Some((6, 9)));
}
```

- [ ] **Step 2: Run tests to verify they FAIL**

```bash
cargo test ollama_client
```
Expected: `resolve_offset_multibyte_valid_char_offsets` and `find_next_occurrence_multibyte_*` FAIL — current code returns byte offsets for multibyte input.

- [ ] **Step 3: Fix `resolve_offset`**

Replace in `src/ollama_client.rs`:

```rust
fn resolve_offset(source: &str, entity: &LlmEntity, used: &[(usize, usize)]) -> Option<(usize, usize)> {
    if entity.text.is_empty() {
        return None;
    }

    // step 1: validate LLM-provided char offsets by slicing with byte conversion
    let llm_range = (entity.start, entity.end);
    let start_byte = crate::unicode::char_to_byte(source, entity.start);
    let end_byte   = crate::unicode::char_to_byte(source, entity.end);
    if source.get(start_byte..end_byte) == Some(entity.text.as_str())
        && !overlaps_any(llm_range, used)
    {
        return Some(llm_range); // already char offsets
    }

    // step 2: text-search fallback — returns char offsets
    find_next_occurrence(source, &entity.text, used)
}
```

- [ ] **Step 4: Fix `find_next_occurrence`**

Replace in `src/ollama_client.rs`:

```rust
fn find_next_occurrence(source: &str, needle: &str, used: &[(usize, usize)]) -> Option<(usize, usize)> {
    source
        .match_indices(needle)
        .find_map(|(byte_start, s)| {
            let char_start = crate::unicode::byte_to_char(source, byte_start);
            let char_end   = crate::unicode::byte_to_char(source, byte_start + s.len());
            if !overlaps_any((char_start, char_end), used) {
                Some((char_start, char_end))
            } else {
                None
            }
        })
}
```

- [ ] **Step 5: Run tests to verify they PASS**

```bash
cargo test ollama_client
```
Expected: all ollama_client tests PASS.

- [ ] **Step 6: Run full test suite**

```bash
cargo test
```
Expected: all tests PASS.

- [ ] **Step 7: Commit**

```bash
git add src/ollama_client.rs
git commit -m "fix(ollama_client): resolve and return char offsets for Unicode text"
```

---

## Verify (end-to-end)

After all three tasks are committed, rebuild and run on BUG-A samples to confirm the fix:

```bash
cargo build --release

# Quick sanity check: Python char offsets for pii_location_person_it::006
python3 - <<'EOF'
import json
data = json.load(open("test-data/pii_location_person_it.json"))
s = next(x for x in data["samples"] if x["id"] == "006")
for e in s["expected"]:
    print(f"  {e['entity_type']} [{e['start']}:{e['end']}] = {s['text'][e['start']:e['end']]!r}")
EOF
# Expected: correct substrings (no leading spaces, no truncation)
```

The BUG-A entries (`" Torin"`, `" Milan"`, `" chiara.lombardi..."`) will disappear from the report errors — those samples were already annotated with correct char offsets; only Rust's interpretation was wrong.

Run Ollama eval to confirm improvement:
```bash
cargo run -- --backend ollama \
             --analyzer-url http://localhost:11434/api/chat \
             --ollama-model qwen2.5:7b-instruct-q4_K_M \
             --input ./test-data --verbose
```

Expected: NearMiss errors for `pii_location_person_it::006`, `pii_vat_fiscal_it::003`, `pii_vat_fiscal_it::004` disappear or become strict TPs.
