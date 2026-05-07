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
    pub presidio_expected: Vec<ExpectedEntity>,
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

/// Predicted/expected pair that overlaps in type+span but not exactly.
#[derive(Debug, Clone, Serialize)]
pub struct NearMiss {
    pub obtained: PredictedSpan,
    pub expected: ExpectedEntity,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestError {
    pub sample_id: String,
    pub severity: ErrorSeverity,
    pub kinds: Vec<ErrorKind>,
    pub near_misses: Vec<NearMiss>,
    pub false_positives: Vec<PredictedSpan>,
    pub false_negatives: Vec<ExpectedEntity>,
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
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub f2: f64,
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
}

#[derive(Debug, Serialize)]
pub struct RunParams {
    pub input: String,
    pub analyzer_url: String,
    pub output: String,
    pub recursive: bool,
    pub verbose: bool,
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
