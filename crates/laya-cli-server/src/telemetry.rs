use std::{
    collections::VecDeque,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

const RETAINED_REQUESTS: usize = 100;

#[derive(Debug, Clone, Serialize)]
pub struct RequestEntry {
    pub id: u64,
    pub at_unix_ms: u64,
    pub status: &'static str,
    pub duration_ms: u64,
    pub question_count: usize,
}

#[derive(Serialize)]
pub struct Stats {
    pub total_requests: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub average_latency_ms: f64,
}

#[derive(Serialize)]
pub struct RequestPage {
    pub items: Vec<RequestEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Default)]
pub struct Telemetry {
    succeeded: u64,
    failed: u64,
    total_duration: Duration,
    recent: VecDeque<RequestEntry>,
}

impl Telemetry {
    pub fn record(&mut self, succeeded: bool, duration: Duration, question_count: usize) {
        if succeeded {
            self.succeeded += 1;
        } else {
            self.failed += 1;
        }
        self.total_duration += duration;
        let id = self.succeeded + self.failed;
        let at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if self.recent.len() == RETAINED_REQUESTS {
            self.recent.pop_front();
        }
        self.recent.push_back(RequestEntry {
            id,
            at_unix_ms,
            status: if succeeded { "ok" } else { "error" },
            duration_ms: duration.as_millis() as u64,
            question_count,
        });
    }

    pub fn stats(&self) -> Stats {
        let total_requests = self.succeeded + self.failed;
        Stats {
            total_requests,
            succeeded: self.succeeded,
            failed: self.failed,
            average_latency_ms: if total_requests == 0 {
                0.0
            } else {
                self.total_duration.as_secs_f64() * 1000.0 / total_requests as f64
            },
        }
    }

    pub fn page(&self, limit: usize, before: Option<u64>) -> RequestPage {
        let mut entries = self
            .recent
            .iter()
            .rev()
            .filter(|entry| before.is_none_or(|id| entry.id < id));
        let items: Vec<_> = entries.by_ref().take(limit).cloned().collect();
        let next_cursor = if entries.next().is_some() {
            items.last().map(|entry| entry.id.to_string())
        } else {
            None
        };
        RequestPage { items, next_cursor }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_is_stable_and_history_is_bounded() {
        let mut history = Telemetry::default();
        for i in 0..105 {
            history.record(i % 2 == 0, Duration::from_millis(10), i);
        }
        let stats = history.stats();
        assert_eq!(
            (stats.total_requests, stats.succeeded, stats.failed),
            (105, 53, 52)
        );
        assert_eq!(stats.average_latency_ms, 10.0);
        let first = history.page(50, None);
        assert_eq!(first.items.first().unwrap().id, 105);
        assert_eq!(first.items.last().unwrap().id, 56);
        assert_eq!(first.next_cursor.as_deref(), Some("56"));
        let second = history.page(50, Some(56));
        assert_eq!(second.items.first().unwrap().id, 55);
        assert_eq!(second.items.last().unwrap().id, 6);
        assert_eq!(second.next_cursor, None);
    }
}
