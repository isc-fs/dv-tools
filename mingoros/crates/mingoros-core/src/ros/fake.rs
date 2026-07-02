//! In-process fake ROS graph — the hardware-free backend.
//!
//! Publishes synthetic, contract-accurate samples for every topic in
//! [`dv_contract::KNOWN_TOPICS`], so `mingoros topics` / `echo` / `hz` work
//! with no ROS install and no live pipeline. This is both a development
//! convenience and the seed of the "ROS-side fake publisher" the design calls
//! for (drives the visualizer with no car). Deterministic: values vary by
//! sequence number only (no RNG), so output is reproducible.

use super::{RosClient, RosError, Sample, SampleStream, SetBoolOutcome, TopicInfo};
use crate::dv_contract::{self, AsState, DvStatus};
use std::time::{Duration, Instant};

/// A fake ROS client backed purely by [`dv_contract::KNOWN_TOPICS`].
#[derive(Debug, Default)]
pub struct FakeRos;

impl FakeRos {
    pub fn new() -> Self {
        Self
    }
}

impl RosClient for FakeRos {
    fn backend_name(&self) -> &'static str {
        "fake"
    }

    fn list_topics(&self) -> Result<Vec<TopicInfo>, RosError> {
        Ok(dv_contract::KNOWN_TOPICS
            .iter()
            .map(TopicInfo::from_spec)
            .collect())
    }

    fn subscribe(&self, topic: &str) -> Result<Box<dyn SampleStream>, RosError> {
        let spec = dv_contract::KNOWN_TOPICS
            .iter()
            .find(|s| s.name == topic)
            .ok_or_else(|| RosError::TopicNotFound(topic.to_string()))?;
        Ok(Box::new(FakeStream {
            topic: spec.name.to_string(),
            type_name: spec.type_name.to_string(),
            interval: cadence_for(spec.name),
            seq: 0,
            start: Instant::now(),
        }))
    }

    fn publish(&self, topic: &str, value: &str) -> Result<(), RosError> {
        if dv_contract::KNOWN_TOPICS.iter().all(|s| s.name != topic) {
            // Unknown topic is allowed on a real graph; the fake just notes it.
            tracing::warn!(topic, "publish to unknown topic (fake accepts it)");
        }
        tracing::info!(topic, value, "fake publish");
        Ok(())
    }

    fn call_set_bool(&self, service: &str, data: bool) -> Result<SetBoolOutcome, RosError> {
        tracing::info!(service, data, "fake SetBool service call");
        let message = if service == dv_contract::SERVICE_FORCE_EBS {
            if data {
                "EBS forced open (simulated — fake backend)".to_string()
            } else {
                "EBS returned to normal (simulated — fake backend)".to_string()
            }
        } else {
            format!("{service} data={data} accepted (simulated — fake backend)")
        };
        Ok(SetBoolOutcome {
            success: true,
            message,
        })
    }
}

/// Synthetic sample cadence per topic (demo-friendly, not the real rate).
fn cadence_for(topic: &str) -> Duration {
    match topic {
        dv_contract::TOPIC_IMU => Duration::from_millis(50),
        dv_contract::TOPIC_ASSI_STATE
        | dv_contract::TOPIC_DV_STATUS
        | dv_contract::TOPIC_CTRL_CMD
        | dv_contract::TOPIC_SLAM_POSE
        | dv_contract::TOPIC_ODOM => Duration::from_millis(100),
        _ => Duration::from_millis(200),
    }
}

struct FakeStream {
    topic: String,
    type_name: String,
    interval: Duration,
    seq: u64,
    start: Instant,
}

impl SampleStream for FakeStream {
    fn next_sample(&mut self) -> Option<Sample> {
        std::thread::sleep(self.interval);
        let seq = self.seq;
        self.seq += 1;
        Some(Sample {
            topic: self.topic.clone(),
            type_name: self.type_name.clone(),
            seq,
            t_ms: self.start.elapsed().as_millis(),
            summary: synth_summary(&self.topic, seq),
        })
    }
}

/// Contract-accurate synthetic payload summary for a topic + sequence.
fn synth_summary(topic: &str, seq: u64) -> String {
    match topic {
        dv_contract::TOPIC_ASSI_STATE => {
            // OFF → READY → DRIVING → DRIVING… → FINISHED (a plausible run).
            let s = match seq % 8 {
                0 => AsState::Off,
                1 => AsState::Ready,
                7 => AsState::Finished,
                _ => AsState::Driving,
            };
            format!("data: {} ({})", s.as_u8(), s.label())
        }
        dv_contract::TOPIC_DV_STATUS => {
            let s = match seq % 8 {
                0 => DvStatus::Idle,
                1 => DvStatus::Preparing,
                2 => DvStatus::Ready,
                7 => DvStatus::Finished,
                _ => DvStatus::Running,
            };
            format!("data: {} ({})", s.as_u8(), s.label())
        }
        dv_contract::TOPIC_AS_STATE => {
            let s = match seq % 8 {
                0 => dv_contract::RawAsState::Off,
                1 => dv_contract::RawAsState::Ready,
                7 => dv_contract::RawAsState::Finished,
                _ => dv_contract::RawAsState::Driving,
            };
            format!("data: {} ({})", s as u8, s.label())
        }
        dv_contract::TOPIC_RES_STATUS => {
            let (v, r) = if seq >= 3 {
                (2, dv_contract::ResStatus::Go)
            } else {
                (0, dv_contract::ResStatus::Ok)
            };
            format!("data: {v} ({})", r.label())
        }
        dv_contract::TOPIC_RES_GO => {
            let go = seq >= 3;
            format!("data: {} ({})", go as i32, if go { "GO" } else { "no-GO" })
        }
        dv_contract::TOPIC_AMI_MISSION => {
            let ami = (seq % 7) as i32; // 0..6
            let mid = dv_contract::ami_index_to_mission_id(ami);
            let name = dv_contract::Mission::from_id(mid)
                .map(|m| m.name())
                .unwrap_or("none");
            format!("data: {ami}  (→ mission_id {mid} {name})")
        }
        dv_contract::TOPIC_DEBUG => {
            // A plausible safety dashboard: ASMS+TS+R2D+standstill latch on as
            // the sequence advances, EBS stays off — like a stopped bring-up.
            use dv_contract::state_signal as s;
            let mut sig = s::SDC_RES_OPEN | s::MISSION_SEL;
            if seq >= 1 {
                sig |= s::ASMS_ON | s::TS_ACTIVE | s::STANDSTILL;
                sig &= !s::SDC_RES_OPEN;
            }
            if seq >= 3 {
                sig |= s::ABS_CHECKS_OK | s::R2D;
            }
            let as_name = if seq >= 3 { "AS_READY" } else { "AS_OFF" };
            format!(
                "AS {as_name} || {} || RES:{}",
                dv_contract::describe_state_signals(sig),
                if seq >= 3 { "GO" } else { "OK" }
            )
        }
        dv_contract::TOPIC_CTRL_CMD => {
            let t = ((seq as f64) * 0.2).sin();
            let s = ((seq as f64) * 0.1).cos() * 0.3;
            format!("linear.x: {t:+.3}  angular.z: {s:+.3}")
        }
        dv_contract::TOPIC_IMU => {
            let ax = ((seq as f64) * 0.05).sin() * 2.0;
            let wz = ((seq as f64) * 0.05).cos() * 0.5;
            format!("accel.x: {ax:+.2} m/s²  gyro.z: {wz:+.2} rad/s")
        }
        dv_contract::TOPIC_STEERING => {
            let a = ((seq as f64) * 0.1).sin() * 0.35;
            format!("data: {a:+.3} rad")
        }
        dv_contract::TOPIC_MOTOR_RPM => {
            let rpm = 800.0 + ((seq as f64) * 0.1).sin().abs() * 4000.0;
            format!("data: {rpm:.0} rpm")
        }
        dv_contract::TOPIC_SLAM_POSE | dv_contract::TOPIC_ODOM => {
            let x = (seq as f64) * 0.15;
            let y = ((seq as f64) * 0.05).sin() * 1.2;
            let yaw = ((seq as f64) * 0.03).sin();
            format!("pose: x={x:.2} y={y:.2} yaw={yaw:+.3}")
        }
        dv_contract::TOPIC_PATH => {
            format!("Path: {} poses ahead", 20 + (seq % 10))
        }
        _ => format!("seq={seq}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_lists_all_known_topics() {
        let ros = FakeRos::new();
        let topics = ros.list_topics().unwrap();
        assert_eq!(topics.len(), dv_contract::KNOWN_TOPICS.len());
        assert!(topics
            .iter()
            .any(|t| t.name == dv_contract::TOPIC_DV_STATUS));
    }

    #[test]
    fn subscribe_unknown_topic_errors() {
        let ros = FakeRos::new();
        assert!(matches!(
            ros.subscribe("/no/such/topic"),
            Err(RosError::TopicNotFound(_))
        ));
    }

    #[test]
    fn dv_status_stream_yields_contract_bytes() {
        let ros = FakeRos::new();
        let mut s = ros.subscribe(dv_contract::TOPIC_DV_STATUS).unwrap();
        let sample = s.next_sample().unwrap();
        assert_eq!(sample.topic, dv_contract::TOPIC_DV_STATUS);
        assert!(sample.summary.starts_with("data: "));
    }
}
