# PLAN — Testo estratto negli errori di output

## Status legend
- `[x]` fatto
- `[~]` in corso
- `[x]` fatto

## Obiettivo

Rendere gli errori leggibili mostrando il testo realmente selezionato da Presidio
(e quello atteso), invece dei soli offset numerici.

### Prima
```
  sample::002  [LOW]
    extra     +  URL       [25-33]
    extra     +  URL       [37-48]
    extra     +  LOCATION  [63-73]
```

### Dopo
```
  sample::002  [LOW]
    extra     +  URL       [25-33]  "mario.ro"
    extra     +  URL       [37-48]  "example.com"
    extra     +  LOCATION  [63-73]  "02 1234567"

  sample::003  [HIGH]
    near miss    EMAIL_ADDRESS  obtained [25-48] "mario.rossi@example.com"
                               expected [28-50] "io.rossi@example.com o"
    missed    -  PERSON    [10-21]  "Mario Rossi"
```

---

## Design

### Problema attuale
`checker::check()` non riceve il testo sorgente → `TestError` contiene solo
offset. `report.rs` non può estrarre il testo a posteriori perché non lo ha.

### Soluzione

Introdurre `SpanWithText` — un tipo usato esclusivamente nel contesto degli
errori, che affianca gli offset al testo estratto:

```rust
// model.rs
pub struct SpanWithText {
    pub entity_type: String,
    pub start: usize,
    pub end: usize,
    pub text: String,
}
```

- `PredictedSpan` e `ExpectedEntity` restano invariati (solo offset, usati
  per il matching).
- `NearMiss`, `TestError.false_positives`, `TestError.false_negatives`
  passano a usare `SpanWithText`.
- `checker::check()` riceve `source_text: &str` e chiama `extract()` solo
  quando costruisce il `TestError` (path di errore, non path hot).

### Perché non aggiungere `text` direttamente a `PredictedSpan`/`ExpectedEntity`?
Quei tipi modellano il contratto HTTP con Presidio e il formato dei file JSON
di test — non devono portare dati derivati. `SpanWithText` è un tipo di
presentazione, separato dal modello di dominio.

---

## File toccati

| File | Modifica |
|------|----------|
| `src/model.rs` | aggiunge `SpanWithText`; `NearMiss` usa `SpanWithText`; `TestError` usa `SpanWithText` per FP e FN |
| `src/checker.rs` | `check()` riceve `source_text: &str`; `build_error()` chiama `extract()`; aggiorna test |
| `src/report.rs` | aggiorna display per mostrare il campo `text` |
| `src/main.rs` | aggiorna la chiamata a `checker::check()` passando `&sample.text` |

---

## Dettaglio modifiche

### `model.rs`

```rust
#[derive(Debug, Clone, Serialize)]
pub struct SpanWithText {
    pub entity_type: String,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

pub struct NearMiss {
    pub obtained: SpanWithText,
    pub expected: SpanWithText,
}

pub struct TestError {
    pub sample_id: String,
    pub severity: ErrorSeverity,
    pub kinds: Vec<ErrorKind>,
    pub near_misses: Vec<NearMiss>,
    pub false_positives: Vec<SpanWithText>,   // era Vec<PredictedSpan>
    pub false_negatives: Vec<SpanWithText>,   // era Vec<ExpectedEntity>
}
```

### `checker.rs`

```rust
pub fn check(
    sample_id: &str,
    source_text: &str,       // ← nuovo parametro
    predicted: &[PredictedSpan],
    expected: &[ExpectedEntity],
) -> CheckResult

fn extract(source: &str, start: usize, end: usize) -> String {
    source.get(start..end).unwrap_or("").to_string()
}
```

`build_error()` usa `extract()` per costruire `SpanWithText` da
`PredictedSpan` e `ExpectedEntity`.

### `report.rs`

```
near miss  EMAIL_ADDRESS  obtained [25-48] "mario.rossi@example.com"
                          expected [28-50] "io.rossi@example.com o"
extra     +  URL  [25-33]  "mario.ro"
missed    -  PERSON  [10-21]  "Mario Rossi"
```

### `main.rs`

```rust
let result = checker::check(&sample.id, &sample.text, &predicted, &sample.presidio_expected);
```

---

## Note sui test

I test esistenti di `checker.rs` passano `""` come `source_text` →
`extract()` restituirà stringhe vuote per i testi, ma i conteggi (tp/fp/fn)
e le asserzioni sulla struttura degli errori rimangono validi.
Aggiungere un test che verifica il testo estratto correttamente.
