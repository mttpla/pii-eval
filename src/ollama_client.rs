use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::analyzer::Analyzer;
use crate::model::PredictedSpan;

// ── Request ────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    stream: bool,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

// ── Response ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ChatResponse {
    message: AssistantMessage,
    done_reason: Option<String>,
}

#[derive(Deserialize)]
struct AssistantMessage {
    content: String,
}

// ── LLM entity output ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LlmEntities {
    entities: Vec<LlmEntity>,
}

#[derive(Deserialize)]
struct LlmEntity {
    entity_type: String,
    text: String,
    start: usize,
    end: usize,
}

// ── Client ─────────────────────────────────────────────────────────────────────

pub struct OllamaClient {
    client: Client,
    url: String,
    model: String,
    system_prompt: String,
}

impl OllamaClient {
    pub fn new(url: &str, model: &str, system_prompt: &str, timeout_secs: u64) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("failed to build HTTP client");
        Self {
            client,
            url: url.to_string(),
            model: model.to_string(),
            system_prompt: system_prompt.to_string(),
        }
    }

    fn call_ollama(&self, text: &str) -> Result<String> {
        let req = ChatRequest {
            model: &self.model,
            messages: vec![
                Message { role: "system", content: &self.system_prompt },
                Message { role: "user",   content: text },
            ],
            stream: false, // required: streaming returns one JSON object per token
        };

        let resp = self
            .client
            .post(&self.url)
            .json(&req)
            .send()
            .with_context(|| format!("cannot reach Ollama at {}", self.url))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            anyhow::bail!("HTTP {} from Ollama: {}", status, body);
        }

        let chat: ChatResponse = resp.json().context("cannot parse Ollama response")?;

        if chat.done_reason.as_deref() != Some("stop") {
            eprintln!("WARNING: Ollama done_reason={:?} — output may be truncated", chat.done_reason);
        }

        Ok(chat.message.content)
    }

    pub fn warmup(&self, timeout_secs: u64) -> Result<()> {
        // A separate client is required: cold-start load time routinely exceeds the
        // normal per-request timeout stored in self.client.
        let warmup_client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("failed to build warmup HTTP client")?;

        eprintln!("warming up Ollama model '{}' (timeout: {}s)…", self.model, timeout_secs);

        let req = ChatRequest {
            model: &self.model,
            messages: vec![
                Message { role: "system", content: &self.system_prompt },
                Message { role: "user", content: "." },
            ],
            stream: false,
        };

        let resp = warmup_client
            .post(&self.url)
            .json(&req)
            .send()
            .with_context(|| format!("warm-up: cannot reach Ollama at {}", self.url))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            anyhow::bail!("warm-up: HTTP {} from Ollama: {}", status, body);
        }

        let _: ChatResponse = resp.json().context("warm-up: cannot parse Ollama response")?;
        // done_reason is intentionally ignored: warmup only needs the model loaded,
        // truncated output is harmless here.

        eprintln!("warm-up complete — model loaded");
        Ok(())
    }
}

impl Analyzer for OllamaClient {
    fn analyze(&self, text: &str, _lang: &str) -> Result<Vec<PredictedSpan>> {
        let content = self.call_ollama(text)?;
        let entities = parse_entities(&content)?;
        Ok(resolve_spans(text, entities))
    }
}

// ── JSON parsing ───────────────────────────────────────────────────────────────

fn parse_entities(content: &str) -> Result<Vec<LlmEntity>> {
    let clean = strip_json_fences(content);
    serde_json::from_str::<LlmEntities>(clean)
        .context("cannot parse LLM entity JSON")
        .map(|r| r.entities)
}

fn strip_json_fences(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```") {
        let after_tag = rest.find('\n').map_or(rest, |i| &rest[i + 1..]);
        let before_close = after_tag.rfind("```").unwrap_or(after_tag.len());
        return after_tag[..before_close].trim();
    }
    s
}

// ── Offset resolution (Option C) ───────────────────────────────────────────────

fn resolve_spans(source: &str, entities: Vec<LlmEntity>) -> Vec<PredictedSpan> {
    let mut used: Vec<(usize, usize)> = Vec::new();
    let mut spans = Vec::new();

    for entity in entities {
        match resolve_offset(source, &entity, &used) {
            Some((start, end)) => {
                used.push((start, end));
                spans.push(PredictedSpan { entity_type: entity.entity_type, start, end });
            }
            None => {
                eprintln!(
                    "WARNING: entity '{}' ({}) not found in source text, skipping",
                    entity.text, entity.entity_type
                );
            }
        }
    }

    spans
}

fn resolve_offset(source: &str, entity: &LlmEntity, used: &[(usize, usize)]) -> Option<(usize, usize)> {
    if entity.text.is_empty() {
        return None;
    }

    // step 1: validate LLM-provided offsets (entity.start/end are char offsets)
    let llm_range = (entity.start, entity.end);
    let start_byte = crate::unicode::char_to_byte(source, entity.start);
    let end_byte   = crate::unicode::char_to_byte(source, entity.end);
    if source.get(start_byte..end_byte) == Some(entity.text.as_str())
        && !overlaps_any(llm_range, used)
    {
        return Some(llm_range); // already char offsets, return as-is
    }

    // step 2: text-search fallback
    find_next_occurrence(source, &entity.text, used)
}

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

fn overlaps_any(range: (usize, usize), used: &[(usize, usize)]) -> bool {
    used.iter().any(|&(us, ue)| range.0 < ue && range.1 > us)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(entity_type: &str, text: &str, start: usize, end: usize) -> LlmEntity {
        LlmEntity { entity_type: entity_type.to_string(), text: text.to_string(), start, end }
    }

    // -- strip_json_fences

    #[test]
    fn strip_fences_no_fences() {
        assert_eq!(strip_json_fences(r#"{"entities":[]}"#), r#"{"entities":[]}"#);
    }

    #[test]
    fn strip_fences_json_tagged() {
        assert_eq!(strip_json_fences("```json\n{\"entities\":[]}\n```"), "{\"entities\":[]}");
    }

    #[test]
    fn strip_fences_untagged() {
        assert_eq!(strip_json_fences("```\n{\"entities\":[]}\n```"), "{\"entities\":[]}");
    }

    #[test]
    fn strip_fences_trims_outer_whitespace() {
        assert_eq!(strip_json_fences("  {\"entities\":[]}  "), "{\"entities\":[]}");
    }

    // -- overlaps_any

    #[test]
    fn no_overlap_returns_false() {
        assert!(!overlaps_any((20, 30), &[(0, 10), (40, 50)]));
    }

    #[test]
    fn adjacent_ranges_do_not_overlap() {
        assert!(!overlaps_any((10, 20), &[(0, 10)]));
        assert!(!overlaps_any((0, 10), &[(10, 20)]));
    }

    #[test]
    fn contained_range_overlaps() {
        assert!(overlaps_any((5, 15), &[(0, 20)]));
    }

    #[test]
    fn partial_overlap() {
        assert!(overlaps_any((5, 15), &[(10, 25)]));
    }

    #[test]
    fn empty_used_never_overlaps() {
        assert!(!overlaps_any((0, 100), &[]));
    }

    // -- find_next_occurrence

    #[test]
    fn finds_first_occurrence_when_none_used() {
        let src = "Mario Rossi chiama Mario Rossi";
        assert_eq!(find_next_occurrence(src, "Mario Rossi", &[]), Some((0, 11)));
    }

    #[test]
    fn skips_used_span_finds_second_occurrence() {
        let src = "Mario Rossi chiama Mario Rossi";
        assert_eq!(find_next_occurrence(src, "Mario Rossi", &[(0, 11)]), Some((19, 30)));
    }

    #[test]
    fn needle_not_in_source_returns_none() {
        assert_eq!(find_next_occurrence("hello world", "Mario Rossi", &[]), None);
    }

    // -- resolve_offset

    #[test]
    fn valid_llm_offsets_used_directly() {
        let src = "Mario Rossi said hello";
        let e = entity("PERSON", "Mario Rossi", 0, 11);
        assert_eq!(resolve_offset(src, &e, &[]), Some((0, 11)));
    }

    #[test]
    fn invalid_offsets_fall_back_to_text_search() {
        let src = "Mario Rossi said hello";
        let e = entity("PERSON", "Mario Rossi", 99, 110); // out of range
        assert_eq!(resolve_offset(src, &e, &[]), Some((0, 11)));
    }

    #[test]
    fn valid_offset_already_used_falls_back_to_second_occurrence() {
        let src = "Mario Rossi chiama Mario Rossi";
        let e = entity("PERSON", "Mario Rossi", 0, 11);
        assert_eq!(resolve_offset(src, &e, &[(0, 11)]), Some((19, 30)));
    }

    #[test]
    fn empty_entity_text_returns_none() {
        let e = entity("PERSON", "", 0, 0);
        assert_eq!(resolve_offset("hello world", &e, &[]), None);
    }

    // -- resolve_spans

    #[test]
    fn duplicate_entity_text_gets_distinct_offsets() {
        let src = "Mario Rossi chiama Mario Rossi";
        let entities = vec![
            entity("PERSON", "Mario Rossi", 0, 11),
            entity("PERSON", "Mario Rossi", 0, 11), // LLM emits same offsets for both
        ];
        let spans = resolve_spans(src, entities);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].start, 0);
        assert_eq!(spans[0].end, 11);
        assert_eq!(spans[1].start, 19);
        assert_eq!(spans[1].end, 30);
    }

    #[test]
    fn entity_not_found_in_source_is_skipped() {
        let entities = vec![entity("PERSON", "Mario Rossi", 0, 5)];
        let spans = resolve_spans("hello world", entities);
        assert!(spans.is_empty());
    }

    #[test]
    fn mixed_valid_and_invalid_offsets() {
        let src = "Mario Rossi said hello";
        let entities = vec![
            entity("PERSON",   "Mario Rossi", 0,   11),  // valid
            entity("LOCATION", "hello",       999, 1004), // invalid → fallback
        ];
        let spans = resolve_spans(src, entities);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].entity_type, "PERSON");
        assert_eq!(spans[0].start, 0);
        assert_eq!(spans[1].entity_type, "LOCATION");
        assert_eq!(spans[1].start, 17);
        assert_eq!(spans[1].end, 22);
    }

    // -- resolve_offset / find_next_occurrence with multibyte chars

    #[test]
    fn resolve_offset_multibyte_valid_char_offsets() {
        // "Mi chiamo Léa Dubois"
        // "Léa" = chars 10..13
        let src = "Mi chiamo Léa Dubois";
        let e = entity("PERSON", "Léa", 10, 13);
        assert_eq!(resolve_offset(src, &e, &[]), Some((10, 13)));
    }

    #[test]
    fn resolve_offset_multibyte_invalid_offsets_fallback() {
        // LLM emits garbage offsets but correct text → fallback to text search → char offsets
        let src = "Mi chiamo Léa Dubois";
        let e = entity("PERSON", "Léa", 999, 1002);
        assert_eq!(resolve_offset(src, &e, &[]), Some((10, 13)));
    }

    #[test]
    fn find_next_occurrence_multibyte_returns_char_offsets() {
        // "Léa" in "Mi chiamo Léa Dubois": byte 10..14, char 10..13
        // Must return char offsets (10, 13), NOT byte offsets (10, 14)
        let src = "Mi chiamo Léa Dubois";
        assert_eq!(find_next_occurrence(src, "Léa", &[]), Some((10, 13)));
    }

    #[test]
    fn find_next_occurrence_multibyte_skips_used_char_offsets() {
        // "Léa e Léa": first Léa = chars 0..3, second Léa = chars 6..9
        let src = "Léa e Léa";
        assert_eq!(find_next_occurrence(src, "Léa", &[(0, 3)]), Some((6, 9)));
    }

    // -- parse_entities

    #[test]
    fn parses_valid_json() {
        let json = r#"{"entities":[{"entity_type":"PERSON","text":"Mario","start":0,"end":5}]}"#;
        let entities = parse_entities(json).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_type, "PERSON");
        assert_eq!(entities[0].start, 0);
        assert_eq!(entities[0].end, 5);
    }

    #[test]
    fn parses_json_inside_fences() {
        let json = "```json\n{\"entities\":[]}\n```";
        let entities = parse_entities(json).unwrap();
        assert!(entities.is_empty());
    }

    #[test]
    fn parses_empty_entities_array() {
        let entities = parse_entities(r#"{"entities":[]}"#).unwrap();
        assert!(entities.is_empty());
    }

    #[test]
    fn invalid_json_returns_error() {
        assert!(parse_entities("not json at all").is_err());
    }
}

#[cfg(test)]
mod warmup_tests {
    use super::*;
    use mockito::Server;

    fn valid_response_body() -> &'static str {
        r#"{"message":{"content":""},"done_reason":"stop"}"#
    }

    #[test]
    fn warmup_succeeds_on_valid_response() {
        let mut server = Server::new();
        let mock = server
            .mock("POST", "/api/chat")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(valid_response_body())
            .create();

        let url = format!("{}/api/chat", server.url());
        let client = OllamaClient::new(&url, "test-model", "", 5);
        assert!(client.warmup(10).is_ok());
        mock.assert();
    }

    #[test]
    fn warmup_fails_on_http_error() {
        let mut server = Server::new();
        let _mock = server
            .mock("POST", "/api/chat")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let url = format!("{}/api/chat", server.url());
        let client = OllamaClient::new(&url, "test-model", "", 5);
        let err = client.warmup(5).unwrap_err();
        assert!(err.to_string().contains("warm-up: HTTP 500"));
    }

    #[test]
    fn warmup_fails_on_invalid_json() {
        let mut server = Server::new();
        let _mock = server
            .mock("POST", "/api/chat")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("not json at all")
            .create();

        let url = format!("{}/api/chat", server.url());
        let client = OllamaClient::new(&url, "test-model", "", 5);
        let err = client.warmup(5).unwrap_err();
        assert!(err.to_string().contains("warm-up: cannot parse Ollama response"));
    }

    #[test]
    fn warmup_fails_on_unreachable_url() {
        // Bind to :0 to get an OS-allocated port, then drop immediately — guaranteed refused
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let url = format!("http://127.0.0.1:{}/api/chat", port);
        let client = OllamaClient::new(&url, "test-model", "", 1);
        let err = client.warmup(1).unwrap_err();
        assert!(err.to_string().contains("warm-up:"));
    }
}
