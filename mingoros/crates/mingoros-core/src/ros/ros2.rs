//! The `ros2` transport backend — pure-Rust DDS via `ros2-client` / RustDDS.
//!
//! Talks RTPS/DDS over UDP and interoperates with the pipeline's default
//! `rmw_fastrtps`. No ROS install required. This is the transport MingoROS
//! actually uses inside the DV pipeline (Linux/Pi); on the IFSSIM bench it
//! runs in a Linux container joined to the pipeline's DDS domain.
//!
//! Read-only by design for now: publishing onto a safety-critical graph is
//! gated behind the actuation safety gate + a later, deliberate feature.

use super::{RosClient, RosError, Sample, SampleStream, TopicInfo};
use crate::{dv_contract, msgs};
use ros2_client::ros2::{policy, Duration as DdsDuration, QosPolicies, QosPolicyBuilder};
use ros2_client::{Context, MessageTypeName, Name, Node, NodeName, NodeOptions};
use serde::de::DeserializeOwned;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long `next_sample()` waits for a message before giving up (so `echo`
/// on a silent topic returns instead of hanging forever). Generous enough to
/// cover a late-join TRANSIENT_LOCAL durability handshake.
const RECV_TIMEOUT: Duration = Duration::from_secs(20);
/// Poll interval while waiting on the (non-blocking) DDS reader.
const POLL: Duration = Duration::from_millis(20);
/// Discovery settle time after node creation before topics are enumerable.
const DISCOVERY_SETTLE: Duration = Duration::from_secs(3);

pub struct Ros2Client {
    node: Mutex<Node>,
    context: Context,
}

impl Ros2Client {
    /// Join the DDS graph. Domain id comes from `ROS_DOMAIN_ID` (default 0),
    /// matching the pipeline. Spawns the node spinner (required for the
    /// reliability / durability handshakes and to drain node events) on a
    /// background thread, then blocks briefly to let discovery settle.
    pub fn new() -> Result<Self, RosError> {
        let context =
            Context::new().map_err(|e| RosError::Other(format!("DDS context init: {e:?}")))?;
        let mut node = context
            .new_node(
                NodeName::new("/mingoros", "mingoros")
                    .map_err(|e| RosError::Other(format!("node name: {e:?}")))?,
                NodeOptions::new().enable_rosout(false),
            )
            .map_err(|e| RosError::Other(format!("create node: {e:?}")))?;

        // The spinner drives ROS-graph housekeeping and, crucially, lets the
        // DDS readers complete reliable/TRANSIENT_LOCAL exchanges. Run it on a
        // detached thread for the life of the process.
        let spinner = node
            .spinner()
            .map_err(|e| RosError::Other(format!("spinner: {e:?}")))?;
        std::thread::spawn(move || {
            let _ = futures::executor::block_on(spinner.spin());
        });

        std::thread::sleep(DISCOVERY_SETTLE);
        Ok(Self {
            node: Mutex::new(node),
            context,
        })
    }

    fn subscribe_typed<T>(
        &self,
        topic: &str,
        pkg: &str,
        ty: &str,
        qos: QosPolicies,
        fmt: fn(&T) -> String,
    ) -> Result<Box<dyn SampleStream>, RosError>
    where
        T: 'static + Send + DeserializeOwned,
    {
        let mut node = self.node.lock().unwrap();
        let name =
            Name::parse(topic).map_err(|e| RosError::Other(format!("bad topic {topic}: {e:?}")))?;
        let ros_topic = node
            .create_topic(&name, MessageTypeName::new(pkg, ty), &qos)
            .map_err(|e| RosError::Other(format!("create_topic {topic}: {e:?}")))?;
        let sub = node
            .create_subscription::<T>(&ros_topic, Some(qos))
            .map_err(|e| RosError::Other(format!("subscribe {topic}: {e:?}")))?;
        Ok(Box::new(Ros2Stream {
            sub,
            fmt,
            topic: topic.to_string(),
            type_name: format!("{pkg}/msg/{ty}"),
            seq: 0,
            start: Instant::now(),
        }))
    }
}

impl RosClient for Ros2Client {
    fn backend_name(&self) -> &'static str {
        "ros2"
    }

    fn list_topics(&self) -> Result<Vec<TopicInfo>, RosError> {
        let mut seen = std::collections::BTreeMap::new();
        for dt in self.context.discovered_topics() {
            let raw = dt.topic_name();
            // DDS→ROS: user topics are published as "rt/<name>".
            let name = raw
                .strip_prefix("rt")
                .map(str::to_string)
                .unwrap_or_else(|| raw.clone());
            if name.starts_with("ros_discovery_info") || name.is_empty() {
                continue;
            }
            let type_name = demangle_type(dt.type_name());
            seen.entry(name.clone()).or_insert(TopicInfo {
                qos: dv_contract::recommended_qos(&name),
                note: None,
                name,
                type_name,
            });
        }
        Ok(seen.into_values().collect())
    }

    fn subscribe(&self, topic: &str) -> Result<Box<dyn SampleStream>, RosError> {
        match topic {
            // Reliable/volatile small scalars (SLAM diag, control setpoints).
            dv_contract::TOPIC_CONE_SLAM_GT_ERROR
            | dv_contract::TOPIC_CTRL_V_SET
            | dv_contract::TOPIC_CTRL_KAPPA_MAX => self.subscribe_typed::<msgs::Float32>(
                topic,
                "std_msgs",
                "Float32",
                qos_reliable_volatile(),
                |m| format!("data: {:.4}", m.data),
            ),
            // Best-effort high-rate scalars (sim sensor feeds).
            dv_contract::TOPIC_MOTOR_RPM | dv_contract::TOPIC_STEERING => self
                .subscribe_typed::<msgs::Float32>(
                    topic,
                    "std_msgs",
                    "Float32",
                    qos_best_effort(),
                    |m| format!("data: {:.4}", m.data),
                ),
            // THE latched (RELIABLE/TRANSIENT_LOCAL) durability target.
            dv_contract::TOPIC_TESTING_TRACK => {
                self.subscribe_typed::<msgs::Track>(topic, "fs_msgs", "Track", qos_latched(), |m| {
                    format!("Track: {} cones (latched)", m.track.len())
                })
            }
            // Latched std_msgs/Bool topics (+ a bench test topic) — the
            // TRANSIENT_LOCAL durability proof against a retaining writer.
            dv_contract::TOPIC_CTRL_EMERGENCY
            | dv_contract::TOPIC_SLAM_FINISHED
            | "/mingoros_latch" => {
                self.subscribe_typed::<msgs::Bool>(topic, "std_msgs", "Bool", qos_latched(), |m| {
                    format!("data: {} (latched)", m.data)
                })
            }
            // uDV state bytes — decoded to contract labels (the ROS analogue of
            // MingoCAN's DBC decode). BEST_EFFORT to match the uDV publishers.
            dv_contract::TOPIC_ASSI_STATE => self.subscribe_typed::<msgs::UInt8>(
                topic,
                "std_msgs",
                "UInt8",
                qos_best_effort(),
                |m| match dv_contract::AsState::from_u8(m.data) {
                    Some(s) => format!("data: {} ({})", m.data, s.label()),
                    None => format!("data: {} (?)", m.data),
                },
            ),
            dv_contract::TOPIC_AS_STATE => self.subscribe_typed::<msgs::UInt8>(
                topic,
                "std_msgs",
                "UInt8",
                qos_best_effort(),
                |m| match dv_contract::RawAsState::from_u8(m.data) {
                    Some(s) => format!("data: {} ({})", m.data, s.label()),
                    None => format!("data: {} (?)", m.data),
                },
            ),
            // Pipeline→uDV lifecycle byte — latched.
            dv_contract::TOPIC_DV_STATUS => self.subscribe_typed::<msgs::UInt8>(
                topic,
                "std_msgs",
                "UInt8",
                qos_latched(),
                |m| match dv_contract::DvStatus::from_u8(m.data) {
                    Some(s) => format!("data: {} ({}) (latched)", m.data, s.label()),
                    None => format!("data: {} (?) (latched)", m.data),
                },
            ),
            // uDV Int32 topics (BEST_EFFORT); AMI decoded to its mission.
            dv_contract::TOPIC_AMI_MISSION => self.subscribe_typed::<msgs::Int32>(
                topic,
                "std_msgs",
                "Int32",
                qos_best_effort(),
                |m| {
                    let mid = dv_contract::ami_index_to_mission_id(m.data);
                    let name = dv_contract::Mission::from_id(mid)
                        .map(|x| x.name())
                        .unwrap_or("none");
                    format!("data: {} (→ mission_id {mid} {name})", m.data)
                },
            ),
            // RES status — decoded to the coded name (OK/ESTOP/GO/...).
            dv_contract::TOPIC_RES_STATUS => self.subscribe_typed::<msgs::Int32>(
                topic,
                "std_msgs",
                "Int32",
                qos_best_effort(),
                |m| match dv_contract::ResStatus::from_i32(m.data) {
                    Some(r) => format!("data: {} ({})", m.data, r.label()),
                    None => format!("data: {} (?)", m.data),
                },
            ),
            dv_contract::TOPIC_RES_GO => self.subscribe_typed::<msgs::Int32>(
                topic,
                "std_msgs",
                "Int32",
                qos_best_effort(),
                |m| {
                    format!(
                        "data: {} ({})",
                        m.data,
                        if m.data != 0 { "GO" } else { "no-GO" }
                    )
                },
            ),
            // /debug — the uDV safety/state-machine dashboard string
            // (AS state || ASMS/TS/SDC/EBS/ABS || brakes/mission/R2D/... || RES).
            // THE topic to watch when commissioning a stopped car.
            dv_contract::TOPIC_DEBUG => self.subscribe_typed::<msgs::StringMsg>(
                topic,
                "std_msgs",
                "String",
                qos_reliable_volatile(),
                |m| m.data.clone(),
            ),
            // Pose / odometry (RELIABLE) — decoded to x, y, yaw.
            dv_contract::TOPIC_SLAM_POSE
            | dv_contract::TOPIC_ODOM
            | dv_contract::TOPIC_TESTING_ODOM => self.subscribe_typed::<msgs::OdometryPose>(
                topic,
                "nav_msgs",
                "Odometry",
                qos_reliable_volatile(),
                |m| {
                    let p = &m.pose.position;
                    format!(
                        "pose: x={:.2} y={:.2} yaw={:+.3}  (frame {})",
                        p.x,
                        p.y,
                        m.pose.orientation.yaw(),
                        m.header.frame_id
                    )
                },
            ),
            // IMU (BEST_EFFORT) — accel + gyro. (uDV feat/15 and IFSSIM both
            // publish /imu, so TOPIC_IMU covers both.)
            dv_contract::TOPIC_IMU => self.subscribe_typed::<msgs::Imu>(
                topic,
                "sensor_msgs",
                "Imu",
                qos_best_effort(),
                |m| {
                    format!(
                        "accel[{:+.2},{:+.2},{:+.2}] gyro.z={:+.3}",
                        m.linear_acceleration.x,
                        m.linear_acceleration.y,
                        m.linear_acceleration.z,
                        m.angular_velocity.z
                    )
                },
            ),
            // Control command downlink (BEST_EFFORT Twist).
            dv_contract::TOPIC_CTRL_CMD => self.subscribe_typed::<msgs::Twist>(
                topic,
                "geometry_msgs",
                "Twist",
                qos_best_effort(),
                |m| {
                    format!(
                        "throttle(linear.x)={:+.3} steer(angular.z)={:+.3}",
                        m.linear.x, m.angular.z
                    )
                },
            ),
            // fs_msgs/ControlCommand (RELIABLE) — throttle/steering/brake.
            dv_contract::TOPIC_CTRL_CMD_INTERNAL | "/control_command" => self
                .subscribe_typed::<msgs::ControlCommand>(
                    topic,
                    "fs_msgs",
                    "ControlCommand",
                    qos_reliable_volatile(),
                    |m| {
                        format!(
                            "throttle={:+.3} steering={:+.3} brake={:.3}",
                            m.throttle, m.steering, m.brake
                        )
                    },
                ),
            other => Err(RosError::Other(format!(
                "ros2 backend does not yet decode {other} — the QoS spike decodes \
                 std_msgs/Float32 topics and the latched fs_msgs/Track. \
                 (MarkerArray / full type coverage is follow-up work.)"
            ))),
        }
    }

    fn publish(&self, _topic: &str, _value: &str) -> Result<(), RosError> {
        Err(RosError::Other(
            "ros2 backend is read-only for now — publishing onto a live DDS graph \
             is a later, deliberately-gated feature (see the actuation safety gate)"
                .to_string(),
        ))
    }
}

struct Ros2Stream<T> {
    sub: ros2_client::Subscription<T>,
    fmt: fn(&T) -> String,
    topic: String,
    type_name: String,
    seq: u64,
    start: Instant,
}

impl<T: 'static + Send + DeserializeOwned> SampleStream for Ros2Stream<T> {
    fn next_sample(&mut self) -> Option<Sample> {
        let deadline = Instant::now() + RECV_TIMEOUT;
        loop {
            match self.sub.take() {
                Ok(Some((msg, _info))) => {
                    let seq = self.seq;
                    self.seq += 1;
                    return Some(Sample {
                        topic: self.topic.clone(),
                        type_name: self.type_name.clone(),
                        seq,
                        t_ms: self.start.elapsed().as_millis(),
                        summary: (self.fmt)(&msg),
                    });
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        return None;
                    }
                    std::thread::sleep(POLL);
                }
                Err(e) => {
                    tracing::warn!(topic = %self.topic, "take error: {e:?}");
                    return None;
                }
            }
        }
    }
}

fn qos_reliable_volatile() -> QosPolicies {
    QosPolicyBuilder::new()
        .history(policy::History::KeepLast { depth: 10 })
        .reliability(policy::Reliability::Reliable {
            max_blocking_time: DdsDuration::from_millis(100),
        })
        .durability(policy::Durability::Volatile)
        .build()
}

fn qos_best_effort() -> QosPolicies {
    QosPolicyBuilder::new()
        .history(policy::History::KeepLast { depth: 10 })
        .reliability(policy::Reliability::BestEffort)
        .durability(policy::Durability::Volatile)
        .build()
}

/// RELIABLE + TRANSIENT_LOCAL — the latched profile a late joiner needs to
/// still receive the last retained sample of (`/testing_only/track`,
/// `/dv/status`, `/ctrl/emergency`).
fn qos_latched() -> QosPolicies {
    QosPolicyBuilder::new()
        .history(policy::History::KeepLast { depth: 1 })
        .reliability(policy::Reliability::Reliable {
            max_blocking_time: DdsDuration::from_millis(100),
        })
        .durability(policy::Durability::TransientLocal)
        .build()
}

/// DDS type name → ROS type name: `fs_msgs::msg::dds_::Track_` → `fs_msgs/msg/Track`.
fn demangle_type(dds: &str) -> String {
    let s = dds.replace("::dds_::", "::").replace("::", "/");
    s.strip_suffix('_').unwrap_or(&s).to_string()
}
