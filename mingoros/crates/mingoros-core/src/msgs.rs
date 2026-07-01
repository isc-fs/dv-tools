//! Minimal ROS2 message types for the `ros2` backend, as serde structs whose
//! field order matches the `.msg` definitions so RustDDS's CDR (de)serializer
//! reads them correctly off the wire.
//!
//! This is deliberately a small subset — the types the QoS-validation spike
//! actually subscribes to. Full `dv_msgs`/`fs_msgs`/sensor_msgs coverage
//! (esp. `visualization_msgs/MarkerArray` for the cone map) is follow-up work;
//! see ROADMAP.md.

use serde::{Deserialize, Serialize};

/// `std_msgs/Float32` — `float32 data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Float32 {
    pub data: f32,
}

/// `std_msgs/Bool` — `bool data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bool {
    pub data: bool,
}

/// `geometry_msgs/Point` — `float64 x, y, z`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// `fs_msgs/Cone` — `geometry_msgs/Point location; uint8 color`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cone {
    pub location: Point,
    pub color: u8,
}

/// `fs_msgs/Track` — `fs_msgs/Cone[] track`. The latched (TRANSIENT_LOCAL)
/// ground-truth track — the QoS spike's durability target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub track: Vec<Cone>,
}
