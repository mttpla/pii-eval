use std::path::Path;

use anyhow::{Context, Result};

use crate::model::{DualMetrics, ErrorSeverity, EvalReport, MetricSet, RunParams};

const RESET:  &str = "\x1b[0m";
const BOLD:   &str = "\x1b[1m";
const DIM:    &str = "\x1b[2m";
const RED:    &str = "\x1b[31m";
const GREEN:  &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN:   &str = "\x1b[36m";

fn color(value: f64) -> &'static str {
    if value >= 0.85 { GREEN } else if value >= 0.60 { YELLOW } else { RED }
}

fn severity_color(s: &ErrorSeverity) -> &'static str {
    match s {
        ErrorSeverity::High   => RED,
        ErrorSeverity::Medium => YELLOW,
        ErrorSeverity::Low    => DIM,
    }
}

fn print_metric_row(label: &str, m: &MetricSet) {
    if m.precision.is_none() {
        println!(
            "  {DIM}{label:<10}{RESET}  \
             {DIM}P=N/A   R=N/A   F1=N/A   F2=N/A   \
             TP={} FP={} FN={}{RESET}",
            m.tp, m.fp, m.r#fn,
        );
        return;
    }
    let p  = m.precision.unwrap();
    let r  = m.recall.unwrap();
    let f1 = m.f1.unwrap();
    let f2 = m.f2.unwrap();
    println!(
        "  {DIM}{label:<10}{RESET}  \
         P={}{:.3}{RESET}  R={}{:.3}{RESET}  \
         F1={}{:.3}{RESET}  F2={}{:.3}{RESET}  \
         {DIM}TP={} FP={} FN={}{RESET}",
        color(p), p,
        color(r), r,
        color(f1), f1,
        color(f2), f2,
        m.tp, m.fp, m.r#fn,
    );
}

fn print_dual(label: &str, dm: &DualMetrics) {
    if !label.is_empty() {
        println!("  {BOLD}{CYAN}{label}{RESET}");
    }
    print_metric_row("strict",  &dm.strict);
    print_metric_row("relaxed", &dm.relaxed);
}

fn print_params(p: &RunParams) {
    println!("{BOLD}Run params{RESET}");
    println!("  {DIM}input        {RESET} {}", p.input);
    println!("  {DIM}analyzer_url {RESET} {}", p.analyzer_url);
    println!("  {DIM}output       {RESET} {}", p.output);
    println!("  {DIM}recursive    {RESET} {}", p.recursive);
    println!("  {DIM}verbose      {RESET} {}\n", p.verbose);
}

pub fn print_console(report: &EvalReport) {
    let s = &report.summary;

    println!(
        "\n{BOLD}pii-eval{RESET}  {DIM}version {}{RESET}  {DIM}{}{RESET}",
        report.version, report.generated_at,
    );
    println!(
        "{DIM}files={}  samples={}  expected={}  predicted={}  elapsed={}{RESET}\n",
        s.files, s.samples, s.entities_expected, s.entities_predicted, s.elapsed,
    );

    print_params(&report.params);

    println!("{BOLD}Global{RESET}");
    print_dual("", &report.global);

    if !report.by_entity_type.is_empty() {
        println!("\n{BOLD}By entity type{RESET}");
        for (etype, dm) in &report.by_entity_type {
            print_dual(etype, dm);
        }
    }

    if !report.by_language.is_empty() {
        println!("\n{BOLD}By language{RESET}");
        for (lang, dm) in &report.by_language {
            print_dual(lang, dm);
        }
    }

    if !report.test_errors.is_empty() {
        println!(
            "\n{BOLD}{RED}Samples with errors{RESET}  ({} sample(s))",
            report.test_errors.len(),
        );
        for e in &report.test_errors {
            let sc = severity_color(&e.severity);
            let sev = format!("{:?}", e.severity).to_uppercase();
            println!("  {BOLD}{}{RESET}  {sc}[{sev}]{RESET}", e.sample_id);

            for nm in &e.near_misses {
                println!(
                    "    {DIM}near miss{RESET}  {}  \
                     obtained [{}-{}] {YELLOW}\"{}\"{RESET}  \
                     expected [{}-{}] {CYAN}\"{}\"{RESET}",
                    nm.obtained.entity_type,
                    nm.obtained.start, nm.obtained.end, nm.obtained.text,
                    nm.expected.start, nm.expected.end, nm.expected.text,
                );
            }
            for fp in &e.false_positives {
                println!(
                    "    {DIM}extra{RESET}     {RED}+{RESET}  {}  [{}-{}]  {RED}\"{}\"{RESET}",
                    fp.entity_type, fp.start, fp.end, fp.text,
                );
            }
            for fn_ in &e.false_negatives {
                println!(
                    "    {DIM}missed{RESET}    {YELLOW}-{RESET}  {}  [{}-{}]  {YELLOW}\"{}\"{RESET}",
                    fn_.entity_type, fn_.start, fn_.end, fn_.text,
                );
            }
        }
    }

    if !report.api_errors.is_empty() {
        println!(
            "\n{BOLD}{RED}API errors{RESET}  ({} error(s))",
            report.api_errors.len(),
        );
        for e in &report.api_errors {
            println!("  {BOLD}{}{RESET}  {}", e.sample_id, e.message);
            if let Some(body) = &e.server_body {
                println!("    {DIM}{body}{RESET}");
            }
        }
    }

    println!();
}

pub fn write_json(report: &EvalReport, output: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(report).context("cannot serialize report")?;
    std::fs::write(output, json)
        .with_context(|| format!("cannot write {}", output.display()))
}
