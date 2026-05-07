use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

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
    pub fn new(url: &str) -> Self {
        Self { client: Client::new(), url: url.to_string() }
    }

    pub fn analyze(&self, text: &str, language: &str) -> Result<Vec<PredictedSpan>> {
        let resp = self
            .client
            .post(&self.url)
            .json(&AnalyzeRequest { text, language })
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
