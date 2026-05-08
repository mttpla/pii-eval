use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::analyzer::Analyzer;
use crate::model::PredictedSpan;

#[derive(Serialize)]
struct AnalyzeRequest<'a> {
    text: &'a str,
    language: &'a str,
}

#[derive(Deserialize)]
struct RawSpan {
    entity_type: String,
    start: usize,
    end: usize,
}

pub struct PresidioClient {
    client: Client,
    url: String,
}

impl PresidioClient {
    pub fn new(url: &str, timeout_secs: u64) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("failed to build HTTP client");
        Self { client, url: url.to_string() }
    }
}

impl Analyzer for PresidioClient {
    fn analyze(&self, text: &str, lang: &str) -> Result<Vec<PredictedSpan>> {
        let resp = self
            .client
            .post(&self.url)
            .json(&AnalyzeRequest { text, language: lang })
            .send()
            .with_context(|| format!("cannot reach Presidio at {}", self.url))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            anyhow::bail!("HTTP {} from Presidio: {}", status, body);
        }

        let raw: Vec<RawSpan> = resp.json().context("cannot parse Presidio response")?;

        Ok(raw
            .into_iter()
            .map(|s| PredictedSpan { entity_type: s.entity_type, start: s.start, end: s.end })
            .collect())
    }
}
