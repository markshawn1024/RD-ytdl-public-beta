use crate::models::ProgressEvent;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;
use uuid::Uuid;

static PROGRESS_RE: OnceLock<Regex> = OnceLock::new();

fn progress_re() -> &'static Regex {
    PROGRESS_RE.get_or_init(|| {
        Regex::new(
            r"(?x)
            \[download\]\s+
            (?P<percent>\d+(?:\.\d+)?)%
            (?:.*?at\s+(?P<speed>\S+))?
            (?:.*?ETA\s+(?P<eta>\S+))?
            ",
        )
        .expect("progress regex")
    })
}

#[derive(Debug, Clone, Default)]
pub struct ParsedProgress {
    pub percent: Option<f64>,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub destination: Option<String>,
    pub stage: Option<String>,
}

pub fn parse_line(line: &str) -> ParsedProgress {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return ParsedProgress::default();
    }

    // Prefer JSON progress template lines
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            let percent = value.get("percent").and_then(|v| v.as_f64()).or_else(|| {
                value
                    .get("downloaded_bytes")
                    .and_then(|d| d.as_f64())
                    .zip(value.get("total_bytes").and_then(|t| t.as_f64()))
                    .filter(|(_, total)| *total > 0.0)
                    .map(|(downloaded, total)| downloaded / total * 100.0)
            });
            return ParsedProgress {
                percent,
                speed: value
                    .get("speed")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        value
                            .get("speed")
                            .and_then(|v| v.as_f64())
                            .map(|n| format!("{:.1}B/s", n))
                    }),
                eta: value.get("eta").and_then(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| v.as_u64().map(|n| format!("{n}s")))
                }),
                destination: value
                    .get("filename")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                stage: value
                    .get("status")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };
        }
    }

    if let Some(caps) = progress_re().captures(trimmed) {
        return ParsedProgress {
            percent: caps
                .name("percent")
                .and_then(|m| m.as_str().parse::<f64>().ok()),
            speed: caps.name("speed").map(|m| m.as_str().to_string()),
            eta: caps.name("eta").map(|m| m.as_str().to_string()),
            destination: None,
            stage: Some("downloading".into()),
        };
    }

    let mut parsed = ParsedProgress::default();
    if trimmed.contains("[Merger]") || trimmed.contains("Merging formats") {
        parsed.stage = Some("merging".into());
        parsed.percent = Some(99.0);
    } else if trimmed.starts_with("[download] Destination:") {
        parsed.destination = trimmed
            .strip_prefix("[download] Destination:")
            .map(|s| s.trim().to_string());
    } else if trimmed.contains("has already been downloaded") {
        parsed.percent = Some(100.0);
        parsed.stage = Some("done".into());
    }

    parsed
}

pub fn to_event(id: Uuid, parsed: &ParsedProgress) -> Option<ProgressEvent> {
    parsed.percent.map(|percent| ProgressEvent {
        id,
        percent,
        speed: parsed.speed.clone(),
        eta: parsed.eta.clone(),
    })
}
