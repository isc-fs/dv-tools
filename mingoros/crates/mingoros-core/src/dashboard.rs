//! Shared safety-dashboard state — the priority-topic snapshot model used by
//! the terminal (`state`), the web server (`serve`) and the Tauri desktop app.
//!
//! One RX thread per topic updates a shared, freshness-aware snapshot (the
//! WarioCharger dashboard model). Consumers render it however they like.

use crate::dv_contract;
use crate::ros::RosClient;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Priority topics for the stopped-car dashboard: (label, topic).
pub const STATE_TOPICS: &[(&str, &str)] = &[
    ("AS state", dv_contract::TOPIC_ASSI_STATE),
    ("Raw AS", dv_contract::TOPIC_AS_STATE),
    ("DV status", dv_contract::TOPIC_DV_STATUS),
    ("RES", dv_contract::TOPIC_RES_STATUS),
    ("RES go", dv_contract::TOPIC_RES_GO),
    ("Mission", dv_contract::TOPIC_AMI_MISSION),
    ("Safety", dv_contract::TOPIC_DEBUG),
];

/// Latest decoded sample of one topic.
pub struct SignalEntry {
    pub summary: String,
    pub last: Instant,
    pub count: u64,
}

/// Freshness-aware snapshot shared between the RX threads and the renderer.
pub type Snapshot = Arc<Mutex<HashMap<&'static str, SignalEntry>>>;

/// Spawn one RX thread per priority topic, each updating `snap`. Returns the
/// topics the backend couldn't subscribe to. The `client` must outlive the
/// threads (keep it in scope for the dashboard's lifetime).
pub fn spawn_subscribers(client: &dyn RosClient, snap: &Snapshot) -> Vec<&'static str> {
    let mut unavailable = Vec::new();
    for &(_, topic) in STATE_TOPICS {
        match client.subscribe(topic) {
            Ok(mut stream) => {
                let snap = Arc::clone(snap);
                std::thread::spawn(move || {
                    while let Some(s) = stream.next_sample() {
                        // Recover a poisoned lock instead of cascading the panic
                        // into every future dashboard poll (which would crash the app).
                        let mut g = snap.lock().unwrap_or_else(|p| p.into_inner());
                        let e = g.entry(topic).or_insert(SignalEntry {
                            summary: String::new(),
                            last: Instant::now(),
                            count: 0,
                        });
                        e.summary = s.summary;
                        e.last = Instant::now();
                        e.count += 1;
                    }
                });
            }
            Err(_) => unavailable.push(topic),
        }
    }
    unavailable
}

/// A decoded value that signals a danger/fault state on the DV car.
pub fn is_danger(value: &str) -> bool {
    const DANGER: &[&str] = &["EMERGENCY", "ESTOP", "FAILED", "EBS:on"];
    DANGER.iter().any(|d| value.contains(d))
}

/// One row of the rendered dashboard — serialisable for the web/Tauri UIs.
#[derive(Debug, Clone, Serialize)]
pub struct TopicSnapshot {
    pub label: &'static str,
    pub topic: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_ms: Option<u64>,
    pub fresh: bool,
    pub danger: bool,
    /// `"ok"` | `"waiting"` | `"unavailable"`.
    pub state: &'static str,
}

/// Build the current dashboard snapshot (one row per priority topic).
pub fn snapshot(snap: &Snapshot, unavailable: &[&'static str]) -> Vec<TopicSnapshot> {
    let g = snap.lock().unwrap_or_else(|p| p.into_inner());
    let stale = Duration::from_secs_f64(dv_contract::STALENESS_WATCHDOG_S);
    STATE_TOPICS
        .iter()
        .map(|&(label, topic)| match g.get(topic) {
            Some(e) => {
                let age = e.last.elapsed();
                TopicSnapshot {
                    label,
                    topic,
                    value: Some(e.summary.clone()),
                    age_ms: Some(age.as_millis() as u64),
                    fresh: age <= stale,
                    danger: is_danger(&e.summary),
                    state: "ok",
                }
            }
            None => TopicSnapshot {
                label,
                topic,
                value: None,
                age_ms: None,
                fresh: false,
                danger: false,
                state: if unavailable.contains(&topic) {
                    "unavailable"
                } else {
                    "waiting"
                },
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn danger_detection() {
        assert!(is_danger("data: 1 (AS_EMERGENCY)"));
        assert!(is_danger("RES:ESTOP"));
        assert!(is_danger("EBS:on"));
        assert!(!is_danger("data: 2 (AS_READY)"));
        assert!(!is_danger("RES:GO"));
    }
}
