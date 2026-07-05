//! The `ros2` transport backend — pure-Rust DDS via `ros2-client` / RustDDS.
//!
//! Talks RTPS/DDS over UDP and interoperates with the pipeline's default
//! `rmw_fastrtps`. No ROS install required. This is the transport ISC MingoROS
//! actually uses inside the DV pipeline (Linux/Pi); on the IFSSIM bench it
//! runs in a Linux container joined to the pipeline's DDS domain.
//!
//! Read-only by design for now: publishing onto a safety-critical graph is
//! gated behind the actuation safety gate + a later, deliberate feature.

use super::{RosClient, RosError, Sample, SampleStream, SetBoolOutcome, TopicInfo};
use crate::{dv_contract, msgs};
use ros2_client::ros2::{policy, Duration as DdsDuration, QosPolicies, QosPolicyBuilder};
use ros2_client::rustdds::DomainParticipantBuilder;
use ros2_client::{
    AService, Context, ContextOptions, Message, MessageTypeName, Name, Node, NodeName, NodeOptions,
    ServiceMapping, ServiceTypeName,
};
use serde::de::DeserializeOwned;
use std::net::IpAddr;
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

/// How long a `call_set_bool` service call waits for the server's response
/// before giving up (the uDV replies immediately, so a timeout means the
/// service isn't reachable — uDV down / wrong domain / agent not bridging).
const SERVICE_CALL_TIMEOUT: Duration = Duration::from_secs(8);
/// Settle time after creating the service client, to let DDS discovery match it
/// to the server before the first request goes out.
const SERVICE_MATCH_SETTLE: Duration = Duration::from_millis(800);
/// Re-send a still-unanswered request this often — covers the brief window
/// where the client hasn't matched the server yet. `SetBool(true)` is
/// idempotent (it re-asserts the EBS pins), so a duplicate is harmless.
const SERVICE_RESEND: Duration = Duration::from_millis(2500);

// std_srvs/SetBool request+response are `Message`s (Serialize + DeserializeOwned)
// — the trait has no blanket impl, so mark them explicitly. Kept here because
// `ros2_client::Message` only exists under the `ros2` feature.
impl Message for msgs::SetBoolRequest {}
impl Message for msgs::SetBoolResponse {}

pub struct Ros2Client {
    node: Mutex<Node>,
    context: Context,
}

impl Ros2Client {
    /// Join the DDS graph on `domain` (the pipeline uses 0). If `iface` is
    /// given, DDS is bound to ONLY that local interface — the direct-link
    /// Ethernet IP — so discovery multicast + data go over the cable to the DV
    /// PC instead of WiFi (RustDDS has no unicast-peer knob, so pinning the
    /// interface is how a point-to-point link is made to work). Spawns the node
    /// spinner (required for the reliability / durability handshakes and to
    /// drain node events) on a background thread, then blocks briefly to let
    /// discovery settle.
    pub fn new(domain: u16, iface: Option<IpAddr>) -> Result<Self, RosError> {
        let context = match iface {
            Some(ip) => {
                let dp = DomainParticipantBuilder::new(domain)
                    .with_only_networks([ip])
                    .build()
                    .map_err(|e| {
                        RosError::Other(format!(
                            "DDS participant (domain {domain}, bound to {ip}): {e:?}"
                        ))
                    })?;
                Context::from_domain_participant(dp)
                    .map_err(|e| RosError::Other(format!("DDS context: {e:?}")))?
            }
            None => Context::with_options(ContextOptions::new().domain_id(domain))
                .map_err(|e| RosError::Other(format!("DDS context (domain {domain}): {e:?}")))?,
        };
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
            stamp: None,
            topic: topic.to_string(),
            type_name: format!("{pkg}/msg/{ty}"),
            seq: 0,
            start: Instant::now(),
        }))
    }

    /// Like [`subscribe_typed`], but for header-carrying messages: `stamp`
    /// extracts the source timestamp (Unix ms) so each sample also reports the
    /// source→arrival delay (#91).
    fn subscribe_typed_stamped<T>(
        &self,
        topic: &str,
        pkg: &str,
        ty: &str,
        qos: QosPolicies,
        fmt: fn(&T) -> String,
        stamp: fn(&T) -> Option<i64>,
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
            stamp: Some(stamp),
            topic: topic.to_string(),
            type_name: format!("{pkg}/msg/{ty}"),
            seq: 0,
            start: Instant::now(),
        }))
    }

    /// Generic, TYPE-keyed decode for the echo view (vs `subscribe`'s
    /// topic-keyed contract decode): given a topic + its discovered ROS type
    /// name, subscribe with the matching curated struct at BEST_EFFORT (the
    /// widest-matching reader — a best-effort reader matches both reliable and
    /// best-effort writers). Returns `None` if the type has no curated decoder,
    /// so the caller can fall back to a liveness-only subscription.
    fn subscribe_by_type(
        &self,
        topic: &str,
        type_name: &str,
    ) -> Option<Result<Box<dyn SampleStream>, RosError>> {
        let (pkg, ty) = split_ros_type(type_name)?;
        let q = qos_best_effort();
        Some(match (pkg, ty) {
            ("std_msgs", "Bool") => self
                .subscribe_typed::<msgs::Bool>(topic, pkg, ty, q, |m| format!("data: {}", m.data)),
            ("std_msgs", "String") => {
                self.subscribe_typed::<msgs::StringMsg>(topic, pkg, ty, q, |m| m.data.clone())
            }
            ("std_msgs", "Float32") => {
                self.subscribe_typed::<msgs::Float32>(topic, pkg, ty, q, |m| {
                    format!("data: {}", m.data)
                })
            }
            ("std_msgs", "Float64") => {
                self.subscribe_typed::<msgs::Float64>(topic, pkg, ty, q, |m| {
                    format!("data: {}", m.data)
                })
            }
            ("std_msgs", "Int8") => self
                .subscribe_typed::<msgs::Int8>(topic, pkg, ty, q, |m| format!("data: {}", m.data)),
            ("std_msgs", "Int16") => self
                .subscribe_typed::<msgs::Int16>(topic, pkg, ty, q, |m| format!("data: {}", m.data)),
            ("std_msgs", "Int32") => self
                .subscribe_typed::<msgs::Int32>(topic, pkg, ty, q, |m| format!("data: {}", m.data)),
            ("std_msgs", "Int64") => self
                .subscribe_typed::<msgs::Int64>(topic, pkg, ty, q, |m| format!("data: {}", m.data)),
            // Byte/Char are single-octet, same wire form as UInt8.
            ("std_msgs", "UInt8" | "Byte" | "Char") => {
                self.subscribe_typed::<msgs::UInt8>(topic, pkg, ty, q, |m| {
                    format!("data: {}", m.data)
                })
            }
            ("std_msgs", "UInt16") => {
                self.subscribe_typed::<msgs::UInt16>(topic, pkg, ty, q, |m| {
                    format!("data: {}", m.data)
                })
            }
            ("std_msgs", "UInt32") => {
                self.subscribe_typed::<msgs::UInt32>(topic, pkg, ty, q, |m| {
                    format!("data: {}", m.data)
                })
            }
            ("std_msgs", "UInt64") => {
                self.subscribe_typed::<msgs::UInt64>(topic, pkg, ty, q, |m| {
                    format!("data: {}", m.data)
                })
            }
            ("std_msgs", "Header") => {
                self.subscribe_typed::<msgs::Header>(topic, pkg, ty, q, |m| {
                    format!(
                        "frame={} stamp={}.{:09}",
                        m.frame_id, m.stamp.sec, m.stamp.nanosec
                    )
                })
            }
            ("builtin_interfaces", "Time") => {
                self.subscribe_typed::<msgs::Time>(topic, pkg, ty, q, |m| {
                    format!("{}.{:09}", m.sec, m.nanosec)
                })
            }
            ("geometry_msgs", "Vector3") => {
                self.subscribe_typed::<msgs::Vector3>(topic, pkg, ty, q, |m| {
                    format!("x={:.3} y={:.3} z={:.3}", m.x, m.y, m.z)
                })
            }
            ("geometry_msgs", "Point") => {
                self.subscribe_typed::<msgs::Point>(topic, pkg, ty, q, |m| {
                    format!("x={:.3} y={:.3} z={:.3}", m.x, m.y, m.z)
                })
            }
            ("geometry_msgs", "Quaternion") => {
                self.subscribe_typed::<msgs::Quaternion>(topic, pkg, ty, q, |m| {
                    format!(
                        "x={:.3} y={:.3} z={:.3} w={:.3} (yaw={:+.3})",
                        m.x,
                        m.y,
                        m.z,
                        m.w,
                        m.yaw()
                    )
                })
            }
            ("geometry_msgs", "Twist") => {
                self.subscribe_typed::<msgs::Twist>(topic, pkg, ty, q, |m| {
                    format!(
                        "linear[{:+.3},{:+.3},{:+.3}] angular[{:+.3},{:+.3},{:+.3}]",
                        m.linear.x, m.linear.y, m.linear.z, m.angular.x, m.angular.y, m.angular.z
                    )
                })
            }
            ("geometry_msgs", "Accel") => {
                self.subscribe_typed::<msgs::Accel>(topic, pkg, ty, q, |m| {
                    format!(
                        "linear[{:+.3},{:+.3},{:+.3}] angular[{:+.3},{:+.3},{:+.3}]",
                        m.linear.x, m.linear.y, m.linear.z, m.angular.x, m.angular.y, m.angular.z
                    )
                })
            }
            ("geometry_msgs", "Pose") => {
                self.subscribe_typed::<msgs::Pose>(topic, pkg, ty, q, |m| {
                    format!(
                        "pos[{:.3},{:.3},{:.3}] yaw={:+.3}",
                        m.position.x,
                        m.position.y,
                        m.position.z,
                        m.orientation.yaw()
                    )
                })
            }
            ("geometry_msgs", "PoseStamped") => self.subscribe_typed_stamped::<msgs::PoseStamped>(
                topic,
                pkg,
                ty,
                q,
                |m| {
                    format!(
                        "pos[{:.3},{:.3},{:.3}] yaw={:+.3} (frame {})",
                        m.pose.position.x,
                        m.pose.position.y,
                        m.pose.position.z,
                        m.pose.orientation.yaw(),
                        m.header.frame_id
                    )
                },
                |m| header_ms(&m.header),
            ),
            ("nav_msgs", "Odometry") => self.subscribe_typed_stamped::<msgs::OdometryPose>(
                topic,
                pkg,
                ty,
                q,
                |m| {
                    format!(
                        "x={:.2} y={:.2} yaw={:+.3} (frame {})",
                        m.pose.position.x,
                        m.pose.position.y,
                        m.pose.orientation.yaw(),
                        m.header.frame_id
                    )
                },
                |m| header_ms(&m.header),
            ),
            ("sensor_msgs", "Imu") => self.subscribe_typed_stamped::<msgs::Imu>(
                topic,
                pkg,
                ty,
                q,
                |m| {
                    format!(
                        "accel[{:+.2},{:+.2},{:+.2}] gyro.z={:+.3}",
                        m.linear_acceleration.x,
                        m.linear_acceleration.y,
                        m.linear_acceleration.z,
                        m.angular_velocity.z
                    )
                },
                |m| header_ms(&m.header),
            ),
            ("sensor_msgs", "NavSatFix") => {
                self.subscribe_typed::<msgs::NavSatFix>(topic, pkg, ty, q, |m| {
                    format!(
                        "lat={:.7} lon={:.7} alt={:.2}",
                        m.latitude, m.longitude, m.altitude
                    )
                })
            }
            ("sensor_msgs", "Range") => {
                self.subscribe_typed::<msgs::Range>(topic, pkg, ty, q, |m| {
                    format!(
                        "range={:.3} m (fov={:.3} rad, {:.2}–{:.2})",
                        m.range, m.field_of_view, m.min_range, m.max_range
                    )
                })
            }
            ("sensor_msgs", "Temperature") => {
                self.subscribe_typed::<msgs::Temperature>(topic, pkg, ty, q, |m| {
                    format!("{:.2} °C", m.temperature)
                })
            }
            ("sensor_msgs", "FluidPressure") => {
                self.subscribe_typed::<msgs::FluidPressure>(topic, pkg, ty, q, |m| {
                    format!("{:.1} Pa", m.fluid_pressure)
                })
            }
            ("fs_msgs", "ControlCommand") => {
                self.subscribe_typed::<msgs::ControlCommand>(topic, pkg, ty, q, |m| {
                    format!(
                        "throttle={:+.3} steering={:+.3} brake={:.3}",
                        m.throttle, m.steering, m.brake
                    )
                })
            }
            _ => return None,
        })
    }

    /// Liveness-only subscription for a topic whose type has no curated decoder:
    /// register the discovered DDS type (so the reader matches the writer) but
    /// deserialize each sample as `()` (payload ignored). The echo view then
    /// still shows the topic is live + its rate + type name — just not fields.
    fn subscribe_unit(
        &self,
        topic: &str,
        type_name: &str,
    ) -> Result<Box<dyn SampleStream>, RosError> {
        let (pkg, ty) = split_ros_type(type_name)
            .ok_or_else(|| RosError::Other(format!("unrecognised ROS type name {type_name:?}")))?;
        let mut node = self.node.lock().unwrap();
        let name =
            Name::parse(topic).map_err(|e| RosError::Other(format!("bad topic {topic}: {e:?}")))?;
        let q = qos_best_effort();
        let ros_topic = node
            .create_topic(&name, MessageTypeName::new(pkg, ty), &q)
            .map_err(|e| RosError::Other(format!("create_topic {topic}: {e:?}")))?;
        let sub = node
            .create_subscription::<()>(&ros_topic, Some(q))
            .map_err(|e| RosError::Other(format!("subscribe {topic}: {e:?}")))?;
        Ok(Box::new(Ros2Stream {
            sub,
            fmt: |_: &()| "(live — payload not decoded)".to_string(),
            stamp: None,
            topic: topic.to_string(),
            type_name: type_name.to_string(),
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
            // Reliable/volatile control setpoint scalars.
            dv_contract::TOPIC_CTRL_V_SET | dv_contract::TOPIC_CTRL_KAPPA_MAX => self
                .subscribe_typed::<msgs::Float32>(
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
            | dv_contract::TOPIC_TESTING_ODOM => self
                .subscribe_typed_stamped::<msgs::OdometryPose>(
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
                    |m| header_ms(&m.header),
                ),
            // IMU (BEST_EFFORT) — accel + gyro. (uDV feat/15 and IFSSIM both
            // publish /imu, so TOPIC_IMU covers both.)
            dv_contract::TOPIC_IMU => self.subscribe_typed_stamped::<msgs::Imu>(
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
                |m| header_ms(&m.header),
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
                "ros2 backend does not decode {other} yet — decoded types: the uDV state \
                 bytes, RES codes, /debug, nav_msgs/Odometry, sensor_msgs/Imu, \
                 geometry_msgs/Twist, fs_msgs/ControlCommand, std_msgs scalars."
            ))),
        }
    }

    fn subscribe_raw(&self, topic: &str) -> Result<Box<dyn SampleStream>, RosError> {
        // 1. A known DV-contract topic → its rich, topic-specific decode.
        if let Ok(stream) = self.subscribe(topic) {
            return Ok(stream);
        }
        // 2. Otherwise dispatch on the topic's discovered ROS type name.
        let type_name = self
            .topic_info(topic)?
            .map(|t| t.type_name)
            .ok_or_else(|| RosError::TopicNotFound(topic.to_string()))?;
        if let Some(res) = self.subscribe_by_type(topic, &type_name) {
            return res;
        }
        // 3. No curated decoder for this type → liveness only.
        self.subscribe_unit(topic, &type_name)
    }

    fn publish(&self, _topic: &str, _value: &str) -> Result<(), RosError> {
        Err(RosError::Other(
            "ros2 backend is read-only for now — publishing onto a live DDS graph \
             is a later, deliberately-gated feature (see the actuation safety gate)"
                .to_string(),
        ))
    }

    fn call_set_bool(&self, service: &str, data: bool) -> Result<SetBoolOutcome, RosError> {
        let name = Name::parse(service)
            .map_err(|e| RosError::Other(format!("bad service name {service}: {e:?}")))?;
        let qos = qos_service();

        // Create the client under the node lock, then RELEASE the lock before
        // the (potentially multi-second) request/response wait — the RX
        // threads never touch the node lock while polling, but keeping it held
        // would block any concurrent subscribe/connect for the whole call.
        let client = {
            let mut node = self.node.lock().unwrap();
            node.create_client::<AService<msgs::SetBoolRequest, msgs::SetBoolResponse>>(
                ServiceMapping::Enhanced,
                &name,
                &ServiceTypeName::new("std_srvs", "SetBool"),
                qos.clone(),
                qos,
            )
            .map_err(|e| RosError::Other(format!("create service client {service}: {e:?}")))?
        };

        // Let discovery match the client to the uDV's server before the first
        // request, then (re)send until a response arrives or we time out.
        // We track the id of every request we send and accept ONLY a response
        // whose RmwRequestId is one of ours — `receive_response()` can hand back
        // a response addressed to a different client, and a resend puts a second
        // (identical, idempotent) request in flight, so id-matching is required
        // to never mistake someone else's / a stale reply for ours.
        std::thread::sleep(SERVICE_MATCH_SETTLE);
        let deadline = Instant::now() + SERVICE_CALL_TIMEOUT;
        let mut last_send: Option<Instant> = None;
        let mut sent_ids = Vec::new();
        loop {
            if last_send
                .map(|t| t.elapsed() >= SERVICE_RESEND)
                .unwrap_or(true)
            {
                let id = client
                    .send_request(msgs::SetBoolRequest { data })
                    .map_err(|e| RosError::Other(format!("send {service} request: {e:?}")))?;
                sent_ids.push(id);
                last_send = Some(Instant::now());
            }
            match client.receive_response() {
                Ok(Some((id, resp))) => {
                    if sent_ids.contains(&id) {
                        return Ok(SetBoolOutcome {
                            success: resp.success,
                            message: resp.message,
                        });
                    }
                    // A response to a different request — ignore and keep waiting.
                }
                Ok(None) => {}
                Err(e) => {
                    return Err(RosError::Other(format!(
                        "receive {service} response: {e:?}"
                    )))
                }
            }
            if Instant::now() >= deadline {
                // Failure taxonomy (#87): resolve the opaque timeout. A ROS 2
                // service surfaces in DDS discovery as request/reply topics
                // carrying the service name, so we can tell "never advertised"
                // (uDV/agent down / wrong domain) from "advertised but silent"
                // (mapping mismatch / link drop). RustDDS has no service-server
                // match probe, so advertisement is topic-inferred.
                let svc = service.trim_start_matches('/');
                let advertised = self
                    .context
                    .discovered_topics()
                    .iter()
                    .any(|dt| dt.topic_name().contains(svc));
                let secs = SERVICE_CALL_TIMEOUT.as_secs();
                let n = sent_ids.len();
                return Err(RosError::Other(if advertised {
                    format!(
                        "{service}: advertised on the graph but no response in {secs}s \
                         ({n} request(s) sent) — likely a service-mapping mismatch \
                         (rmw Enhanced vs Basic) or the link dropped mid-call"
                    )
                } else {
                    format!(
                        "{service}: NOT advertised on the graph after {secs}s \
                         ({n} request(s) sent) — the uDV is down, the micro-ROS agent \
                         isn't bridging the service, or you're on the wrong ROS domain"
                    )
                }));
            }
            std::thread::sleep(POLL);
        }
    }
}

struct Ros2Stream<T> {
    sub: ros2_client::Subscription<T>,
    fmt: fn(&T) -> String,
    /// For header-carrying messages: extract the source stamp as Unix ms, so we
    /// can report source→arrival delay (#91). `None` for headerless types.
    stamp: Option<fn(&T) -> Option<i64>>,
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
                    let mut summary = (self.fmt)(&msg);
                    // Source→arrival delay = pipeline latency + laptop↔DV-PC
                    // clock skew (unsynced bench ⇒ read the TREND, not the
                    // absolute). Only when the message carries a non-zero stamp.
                    if let Some(stamp) = self.stamp {
                        if let Some(src_ms) = stamp(&msg) {
                            let now_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis() as i64)
                                .unwrap_or(0);
                            summary = format!("{summary}  · srcΔ {}ms", now_ms - src_ms);
                        }
                    }
                    return Some(Sample {
                        topic: self.topic.clone(),
                        type_name: self.type_name.clone(),
                        seq,
                        t_ms: self.start.elapsed().as_millis(),
                        summary,
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
/// still receive the last retained sample of (`/dv/status`, `/ctrl/emergency`,
/// `/slam/finished`).
fn qos_latched() -> QosPolicies {
    QosPolicyBuilder::new()
        .history(policy::History::KeepLast { depth: 1 })
        .reliability(policy::Reliability::Reliable {
            max_blocking_time: DdsDuration::from_millis(100),
        })
        .durability(policy::Durability::TransientLocal)
        .build()
}

/// Service request/response QoS — RELIABLE + KeepLast(1), the profile ROS 2
/// services use by default (rmw_fastrtps), which the micro-ROS agent bridges.
fn qos_service() -> QosPolicies {
    QosPolicyBuilder::new()
        .history(policy::History::KeepLast { depth: 1 })
        .reliability(policy::Reliability::Reliable {
            max_blocking_time: DdsDuration::from_millis(100),
        })
        .build()
}

/// A `std_msgs/Header` stamp → Unix ms, or `None` when unstamped (sec=0,
/// nanosec=0) so we don't report a bogus multi-decade delay.
fn header_ms(h: &msgs::Header) -> Option<i64> {
    if h.stamp.sec == 0 && h.stamp.nanosec == 0 {
        None
    } else {
        Some(h.stamp.sec as i64 * 1000 + h.stamp.nanosec as i64 / 1_000_000)
    }
}

/// `"std_msgs/msg/UInt8"` → `("std_msgs", "UInt8")` (the `pkg/msg/Type` form
/// produced by [`demangle_type`]). `None` if it isn't a well-formed `msg` name.
fn split_ros_type(type_name: &str) -> Option<(&str, &str)> {
    let mut parts = type_name.split('/');
    let pkg = parts.next()?;
    let mid = parts.next()?;
    let ty = parts.next()?;
    if mid != "msg" || parts.next().is_some() || pkg.is_empty() || ty.is_empty() {
        return None;
    }
    Some((pkg, ty))
}

/// DDS type name → ROS type name: `std_msgs::msg::dds_::UInt8_` → `std_msgs/msg/UInt8`.
fn demangle_type(dds: &str) -> String {
    let s = dds.replace("::dds_::", "::").replace("::", "/");
    s.strip_suffix('_').unwrap_or(&s).to_string()
}
