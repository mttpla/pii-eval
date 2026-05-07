# PLAN — Output filename auto-generato + params nel report

## Status legend
- `[ ]` non iniziato
- `[~]` in corso
- `[x]` fatto

---

## Change A — Output filename opzionale con auto-generazione `[x]`

### Obiettivo
Se `--output` non viene passato, il file si chiama:
`pii-eval-{version}-{YYYY-MM-DD_HH_mm_SS}.json`

Esempio: `pii-eval-29bded1-2026-05-07_19_58_46.json`

### File toccati
- `src/main.rs` only

### Modifiche

**`Args.output`**: da `PathBuf` con default a `Option<PathBuf>` senza default.
```rust
// prima
#[arg(long, default_value = "presidio_eval_report.json")]
output: PathBuf,

// dopo
#[arg(long)]
output: Option<PathBuf>,
```

**Timestamp condiviso**: `now_iso8601()` e il filename devono usare lo stesso istante.
Estrarre `fn now_secs() -> u64` (sposta la logica di `SystemTime`).
Passare `secs: u64` a due formatter separati:
- `fn iso8601(secs: u64) -> String` → `YYYY-MM-DDTHH:MM:SSZ` (per `generated_at`)
- `fn filename_ts(secs: u64) -> String` → `YYYY-MM-DD_HH_mm_SS` (per il filename)

**Risoluzione output** in `main()`:
```rust
let secs = now_secs();
let generated_at = iso8601(secs);
let output_path = args.output.unwrap_or_else(|| {
    PathBuf::from(format!("pii-eval-{}-{}.json", VERSION, filename_ts(secs)))
});
```

### Vincoli
- Il datetime usato per `generated_at` e per il filename deve essere lo stesso (un solo `now_secs()` call).
- Il path risolto (non `None`) deve essere passato a `RunParams` (vedi Change B).

---

## Change B — Params CLI nel report `[x]`

### Obiettivo
Il report (sia stdout che JSON) mostra tutti i parametri usati — quelli passati
esplicitamente e quelli lasciati al default.

### File toccati
- `src/model.rs` — nuova struct `RunParams` + campo in `EvalReport`
- `src/main.rs` — populate `RunParams`
- `src/report.rs` — stampa sezione params su stdout

### Modifiche

**`model.rs`** — aggiungere dopo `ReportSummary`:
```rust
#[derive(Debug, Serialize)]
pub struct RunParams {
    pub input: String,
    pub analyzer_url: String,
    pub output: String,      // path risolto (mai None)
    pub recursive: bool,
    pub verbose: bool,
}
```

Aggiungere campo a `EvalReport`:
```rust
pub struct EvalReport {
    pub version: String,
    pub generated_at: String,
    pub params: RunParams,   // ← nuovo, dopo generated_at
    pub summary: ReportSummary,
    // ...
}
```

**`main.rs`** — dopo aver risolto `output_path`:
```rust
let params = RunParams {
    input:        args.input.display().to_string(),
    analyzer_url: args.analyzer_url.clone(),
    output:       output_path.display().to_string(),
    recursive:    args.recursive,
    verbose:      args.verbose,
};
```

**`report.rs`** — aggiungere sezione `print_console` prima del Global:
```
Run params
  input          ./test-data
  analyzer_url   http://localhost:5002/analyze
  output         pii-eval-29bded1-2026-05-07_19_58_46.json
  recursive      false
  verbose        false
```

---

## Ordine di esecuzione
1. Change A (main.rs) — autonoma, nessuna dipendenza
2. Change B (model.rs → main.rs → report.rs) — dipende da A solo per `output_path`

## Rollback
Entrambe le change sono additive: Change A cambia solo un campo di Args e
due funzioni private; Change B aggiunge un campo a EvalReport (serializzazione
JSON retrocompatibile se si tratta di nuovi file).
