use anyhow::Result;

use crate::model::PredictedSpan;

pub trait Analyzer {
    fn analyze(&self, text: &str, lang: &str) -> Result<Vec<PredictedSpan>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAnalyzer {
        spans: Vec<PredictedSpan>,
    }

    impl Analyzer for MockAnalyzer {
        fn analyze(&self, _text: &str, _lang: &str) -> Result<Vec<PredictedSpan>> {
            Ok(self.spans.clone())
        }
    }

    fn boxed(spans: Vec<PredictedSpan>) -> Box<dyn Analyzer> {
        Box::new(MockAnalyzer { spans })
    }

    fn span(entity_type: &str, start: usize, end: usize) -> PredictedSpan {
        PredictedSpan { entity_type: entity_type.to_string(), start, end }
    }

    #[test]
    fn dynamic_dispatch_returns_spans() {
        let analyzer = boxed(vec![span("PERSON", 0, 5)]);
        let result = analyzer.analyze("hello world", "en").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].entity_type, "PERSON");
        assert_eq!(result[0].start, 0);
        assert_eq!(result[0].end, 5);
    }

    #[test]
    fn dynamic_dispatch_returns_empty() {
        let analyzer = boxed(vec![]);
        let result = analyzer.analyze("no pii here", "en").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn multiple_spans_preserve_order() {
        let analyzer = boxed(vec![span("PERSON", 0, 5), span("EMAIL_ADDRESS", 10, 25)]);
        let result = analyzer.analyze("text", "it").unwrap();
        assert_eq!(result[0].entity_type, "PERSON");
        assert_eq!(result[1].entity_type, "EMAIL_ADDRESS");
    }
}
