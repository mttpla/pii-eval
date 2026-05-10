use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ── Input types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExpectedEntity {
    pub entity_type: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Deserialize)]
pub struct TestSample {
    pub id: String,
    pub lang: String,
    pub text: String,
    #[serde(alias = "presidio_expected")]
    pub expected: Vec<ExpectedEntity>,
}

#[derive(Debug, Deserialize)]
pub struct TestFile {
    pub samples: Vec<TestSample>,
}

// ── Presidio response ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PredictedSpan {
    pub entity_type: String,
    pub start: usize,
    pub end: usize,
}

// ── Error classification ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ErrorKind {
    NearMiss,
    FalsePositive,
    FalseNegative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ErrorSeverity {
    High,
    Medium,
    Low,
}

/// Span with the text extracted from the source — used only in error reporting.
#[derive(Debug, Clone, Serialize)]
pub struct SpanWithText {
    pub entity_type: String,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// Predicted/expected pair that overlaps in type+span but not exactly.
#[derive(Debug, Clone, Serialize)]
pub struct NearMiss {
    pub obtained: SpanWithText,
    pub expected: SpanWithText,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestError {
    pub sample_id: String,
    pub severity: ErrorSeverity,
    pub kinds: Vec<ErrorKind>,
    pub near_misses: Vec<NearMiss>,
    pub false_positives: Vec<SpanWithText>,
    pub false_negatives: Vec<SpanWithText>,
}

// ── Internal accumulation (not serialized) ─────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SampleCounts {
    pub tp_strict: usize,
    pub fp_strict: usize,
    pub fn_strict: usize,
    pub tp_relaxed: usize,
    pub fp_relaxed: usize,
    pub fn_relaxed: usize,
}

// ── Metrics ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize)]
pub struct MetricSet {
    pub tp: usize,
    pub fp: usize,
    pub r#fn: usize,
    pub precision: Option<f64>,
    pub recall: Option<f64>,
    pub f1: Option<f64>,
    pub f2: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DualMetrics {
    pub strict: MetricSet,
    pub relaxed: MetricSet,
}

// ── Report ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub sample_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_body: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReportSummary {
    pub files: usize,
    pub samples: usize,
    pub api_errors: usize,
    pub entities_expected: usize,
    pub entities_predicted: usize,
    pub elapsed: String,
}

#[derive(Debug, Serialize)]
pub struct RunParams {
    pub input: String,
    pub analyzer_url: String,
    pub output: String,
    pub recursive: bool,
    pub verbose: bool,
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ollama_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_content: Option<String>,
    pub timeout_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct EvalReport {
    pub version: String,
    pub generated_at: String,
    pub params: RunParams,
    pub summary: ReportSummary,
    pub global: DualMetrics,
    pub by_entity_type: BTreeMap<String, DualMetrics>,
    pub by_language: BTreeMap<String, DualMetrics>,
    pub test_errors: Vec<TestError>,
    pub api_errors: Vec<ApiError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_params(backend: &str) -> RunParams {
        RunParams {
            input: "./test-data".to_string(),
            analyzer_url: "http://localhost".to_string(),
            output: "out.json".to_string(),
            recursive: false,
            verbose: false,
            backend: backend.to_string(),
            ollama_model: None,
            system_prompt_path: None,
            system_prompt_content: None,
            timeout_secs: 120,
        }
    }

    #[test]
    fn run_params_includes_prompt_content_in_json() {
        let mut p = base_params("ollama");
        p.system_prompt_path = Some("prompts/v1.md".to_string());
        p.system_prompt_content = Some("You are a PII detector.".to_string());
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"system_prompt_content\""));
        assert!(json.contains("You are a PII detector."));
        assert!(json.contains("\"system_prompt_path\""));
    }

    #[test]
    fn run_params_omits_prompt_fields_when_none() {
        let p = base_params("presidio");
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("system_prompt_content"));
        assert!(!json.contains("system_prompt_path"));
    }
}
