/// latency.rs — Latency tracking utility (unchanged, platform-agnostic)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub struct TimestampEntry {
    pub stage: String,
    pub time:  String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LatencyTracker {
    pub sequence_id: String,
    pub timestamps:  Vec<TimestampEntry>,
    pub metadata:    Option<HashMap<String, String>>,
}

impl LatencyTracker {
    pub fn new(sequence_id: String) -> Self {
        Self { sequence_id, timestamps: Vec::new(), metadata: None }
    }

    pub fn sequence_id(&self) -> &str { &self.sequence_id }

    pub fn add_timestamp(&mut self, stage: &str) {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        self.timestamps.push(TimestampEntry { stage: stage.to_string(), time: now });
    }

    pub fn total_latency(&self) -> Option<i64> {
        if self.timestamps.len() < 2 { return None; }
        let times: Result<Vec<_>, _> = self.timestamps.iter()
            .map(|e| chrono::DateTime::parse_from_rfc3339(&e.time))
            .collect();
        if let Ok(times) = times {
            let min = times.iter().min()?;
            let max = times.iter().max()?;
            Some((*max - *min).num_milliseconds())
        } else { None }
    }
}
