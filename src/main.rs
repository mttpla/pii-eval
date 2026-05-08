mod checker;
mod model;
mod presidio_client;
mod report;
mod stats;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use walkdir::WalkDir;

use crate::model::{ApiError, EvalReport, ReportSummary, RunParams, TestFile};
use crate::presidio_client::PresidioClient;
use crate::stats::Stats;

const VERSION: &str = env!("GIT_SHA");

#[derive(Parser, Debug)]
#[command(
    name = "pii-eval",
    about = "Evaluate PII detection quality against a Presidio analyzer",
    version = VERSION,
)]
struct Args {
    #[arg(long, default_value = "./test-data")]
    input: PathBuf,

    #[arg(long, default_value = "http://localhost:5002/analyze")]
    analyzer_url: String,

    #[arg(long)]
    output: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    recursive: bool,

    #[arg(long, short, default_value_t = false)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let start = std::time::Instant::now();

    let secs = now_secs();
    let generated_at = iso8601(secs);
    let output_path = args.output.unwrap_or_else(|| {
        PathBuf::from(format!("pii-eval-{}-{}.json", VERSION, filename_ts(secs)))
    });

    let params = RunParams {
        input:        args.input.display().to_string(),
        analyzer_url: args.analyzer_url.clone(),
        output:       output_path.display().to_string(),
        recursive:    args.recursive,
        verbose:      args.verbose,
    };

    let client = PresidioClient::new(&args.analyzer_url);
    let mut stats = Stats::new();

    let mut file_count: usize = 0;
    let mut sample_count: usize = 0;
    let mut entities_expected: usize = 0;
    let mut entities_predicted: usize = 0;
    let mut test_errors = Vec::new();
    let mut api_errors: Vec<ApiError> = Vec::new();

    let walker = if args.recursive {
        WalkDir::new(&args.input).min_depth(1)
    } else {
        WalkDir::new(&args.input).min_depth(1).max_depth(1)
    };

    for entry in walker {
        let entry = entry.context("error reading directory entry")?;
        let path = entry.path();

        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;

        let test_file: TestFile = serde_json::from_str(&content)
            .with_context(|| format!("invalid JSON in {}", path.display()))?;

        drop(content);
        file_count += 1;

        for mut sample in test_file.samples {
            sample.id = format!("{}::{}", stem, sample.id);
            sample_count += 1;

            match client.analyze(&sample.text, &sample.lang) {
                Ok(predicted) => {
                    if args.verbose {
                        eprintln!("[>] {}", sample.id);
                    }

                    entities_expected += sample.presidio_expected.len();
                    entities_predicted += predicted.len();

                    let result =
                        checker::check(&sample.id, &sample.text, &predicted, &sample.presidio_expected);

                    if args.verbose {
                        let c = &result.counts;
                        eprintln!(
                            "    strict TP={} FP={} FN={}  relaxed TP={} FP={} FN={}",
                            c.tp_strict, c.fp_strict, c.fn_strict,
                            c.tp_relaxed, c.fp_relaxed, c.fn_relaxed,
                        );
                    }

                    stats.add(&result.counts, &result.by_type, &sample.lang);
                    if let Some(err) = result.error {
                        test_errors.push(err);
                    }
                }
                Err(e) => {
                    let message = e.to_string();
                    eprintln!("ERROR  {}  {}", sample.id, message);
                    api_errors.push(ApiError {
                        sample_id: sample.id,
                        message,
                        server_body: None,
                    });
                }
            }
        }
        // test_file dropped here — only one file in memory at a time
    }

    if sample_count == 0 {
        eprintln!("No samples found in {}", args.input.display());
        return Ok(());
    }

    let (global, by_entity_type, by_language) = stats.finalize();

    let elapsed_secs = start.elapsed().as_secs();
    let elapsed = format!(
        "{:02}:{:02}:{:02}",
        elapsed_secs / 3600,
        (elapsed_secs % 3600) / 60,
        elapsed_secs % 60,
    );

    let report = EvalReport {
        version: VERSION.to_string(),
        generated_at,
        params,
        summary: ReportSummary {
            files: file_count,
            samples: sample_count,
            api_errors: api_errors.len(),
            entities_expected,
            entities_predicted,
            elapsed,
        },
        global,
        by_entity_type,
        by_language,
        test_errors,
        api_errors,
    };

    report::print_console(&report);
    report::write_json(&report, &output_path)?;
    eprintln!("Report saved to {}", output_path.display());

    Ok(())
}

// ── Timestamp ──────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn iso8601(secs: u64) -> String {
    let (year, month, day, hour, min, sec) = secs_to_parts(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn filename_ts(secs: u64) -> String {
    let (year, month, day, hour, min, sec) = secs_to_parts(secs);
    format!("{year:04}-{month:02}-{day:02}_{hour:02}_{min:02}_{sec:02}")
}

fn secs_to_parts(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let sec = secs % 60;
    let min = (secs / 60) % 60;
    let hour = (secs / 3600) % 24;
    let days = secs / 86400;
    let (year, month, day) = days_to_ymd(days);
    (year, month, day, hour, min, sec)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let dy = if is_leap(year) { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        year += 1;
    }
    let month_days = if is_leap(year) {
        [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for dm in month_days {
        if days < dm { break; }
        days -= dm;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
