//! The ROS transport abstraction — MingoROS's equivalent of can-flasher's
//! `CanBackend` trait. Everything above this layer (CLI commands, the future
//! GUI) talks to a `dyn RosClient` and never knows whether it's driving a
//! pure-Rust DDS client, a micro-ROS agent bridge, or the in-process fake.
//!
//! Backends (planned):
//! - [`fake`] — in-process synthetic graph, always available (this file).
//! - `ros2` — ros2-client / RustDDS, behind the `ros2` cargo feature; added
//!   in feat/2 after the QoS-validation spike. See ROADMAP.
//!
//! The trait is deliberately transport-agnostic and blocking-friendly: a
//! subscription hands back a [`SampleStream`] whose `next_sample()` blocks
//! until the next message. That keeps the core dependency-light (no async
//! runtime required for the CLI); a future GUI can pump a stream on its own
//! task, exactly like can-flasher's streaming-command pattern.

use crate::dv_contract::{self, Qos};
use serde::Serialize;

pub mod fake;
#[cfg(feature = "ros2")]
pub mod ros2;

/// Metadata about a discovered (or a priori known) topic.
#[derive(Debug, Clone, Serialize)]
pub struct TopicInfo {
    pub name: String,
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qos: Option<Qos>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl TopicInfo {
    /// Build from a static [`dv_contract::TopicSpec`].
    pub fn from_spec(spec: &dv_contract::TopicSpec) -> Self {
        Self {
            name: spec.name.to_string(),
            type_name: spec.type_name.to_string(),
            qos: spec.qos(),
            note: Some(spec.note.to_string()),
        }
    }
}

/// One received message, reduced to a human/JSON-friendly summary. The real
/// `ros2` backend will attach the decoded payload; the fake fills `summary`
/// with a synthetic value.
#[derive(Debug, Clone, Serialize)]
pub struct Sample {
    pub topic: String,
    pub type_name: String,
    pub seq: u64,
    /// Milliseconds since the subscription started (monotonic host clock).
    pub t_ms: u128,
    pub summary: String,
}

/// The result of a `std_srvs/SetBool` service call (e.g. `/force_ebs`): the
/// server's `success` flag + optional `message`.
#[derive(Debug, Clone, Serialize)]
pub struct SetBoolOutcome {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RosError {
    #[error("topic not found: {0}")]
    TopicNotFound(String),

    #[error(
        "the ros2 transport is not built into this binary — rebuild with \
         `--features ros2` once the QoS-validation spike lands (see ROADMAP feat/2)"
    )]
    TransportUnavailable,

    #[error("{0}")]
    Other(String),
}

/// A live subscription. `next_sample()` blocks until the next message, or
/// returns `None` when the stream is exhausted / closed.
pub trait SampleStream: Send {
    fn next_sample(&mut self) -> Option<Sample>;
}

/// A connection to a ROS graph (real or fake). The single seam every backend
/// implements — the analogue of can-flasher's `CanBackend`. `Send + Sync` so it
/// can live in a shared Tauri managed cell and be called from any command
/// thread (both backends satisfy this: `FakeRos` is a unit struct, `Ros2Client`
/// is a `Mutex<Node>` + `Context`).
pub trait RosClient: Send + Sync {
    /// Name of the backend, for diagnostics (`"fake"`, `"ros2"`).
    fn backend_name(&self) -> &'static str;

    /// Enumerate topics currently visible on the graph.
    fn list_topics(&self) -> Result<Vec<TopicInfo>, RosError>;

    /// Look up one topic's metadata (default: linear scan of `list_topics`).
    fn topic_info(&self, name: &str) -> Result<Option<TopicInfo>, RosError> {
        Ok(self.list_topics()?.into_iter().find(|t| t.name == name))
    }

    /// Subscribe and receive a blocking stream of samples.
    fn subscribe(&self, topic: &str) -> Result<Box<dyn SampleStream>, RosError>;

    /// Publish a simple value onto a topic (echoed/logged by the fake; the
    /// `ros2` backend will type-check + serialise). Routed through the
    /// actuation safety gate at the CLI/GUI layer for command topics.
    fn publish(&self, topic: &str, value: &str) -> Result<(), RosError>;

    /// Call a `std_srvs/SetBool` service (request → response), blocking until
    /// the response arrives or the call times out. This is the seam for the
    /// uDV's actuation services — notably `/force_ebs` (engage the Emergency
    /// Brake System for a car-on-stands checkup). Always gate it behind the
    /// actuation safety gate at the CLI / GUI layer.
    fn call_set_bool(&self, service: &str, data: bool) -> Result<SetBoolOutcome, RosError>;
}
