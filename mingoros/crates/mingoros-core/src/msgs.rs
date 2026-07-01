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

/// `std_msgs/UInt8` — `uint8 data`. The uDV state bytes (`/assi/state`,
/// `/as_state`, `/dv/status`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UInt8 {
    pub data: u8,
}

/// `std_msgs/Int32` — `int32 data`. uDV `/ami/mission`, `/res/*`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Int32 {
    pub data: i32,
}

/// `std_msgs/String` — `string data`. The uDV `/debug` safety/state dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringMsg {
    pub data: String,
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

// --- geometry / header building blocks ---

/// `builtin_interfaces/Time`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Time {
    pub sec: i32,
    pub nanosec: u32,
}

/// `std_msgs/Header`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub stamp: Time,
    pub frame_id: String,
}

/// `geometry_msgs/Quaternion`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Quaternion {
    /// Yaw (rotation about z), radians — the useful DOF for a planar car pose.
    pub fn yaw(&self) -> f64 {
        (2.0 * (self.w * self.z + self.x * self.y))
            .atan2(1.0 - 2.0 * (self.y * self.y + self.z * self.z))
    }
}

/// `geometry_msgs/Vector3`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// `geometry_msgs/Pose`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pose {
    pub position: Point,
    pub orientation: Quaternion,
}

/// `geometry_msgs/Twist`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Twist {
    pub linear: Vector3,
    pub angular: Vector3,
}

// --- prefix structs: read only the leading fields, ignore the trailing
// covariance arrays. Valid because these are top-level (not sequence) messages
// — CDR reads the defined fields in order and leaves the rest of the buffer
// unread, which sidesteps serde's lack of `[f64; 36]` array support. ---

/// `nav_msgs/Odometry` **prefix** — up to `pose.pose`; the `[f64;36]`
/// covariance and the `twist` block are intentionally left unread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdometryPose {
    pub header: Header,
    pub child_frame_id: String,
    pub pose: Pose,
}

/// `sensor_msgs/Imu` **prefix** — up to `linear_acceleration`; the trailing
/// `linear_acceleration_covariance` (`[f64;9]`) is left unread. The two
/// interior `[f64;9]` covariances are read (serde supports arrays ≤ 32).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Imu {
    pub header: Header,
    pub orientation: Quaternion,
    pub orientation_covariance: [f64; 9],
    pub angular_velocity: Vector3,
    pub angular_velocity_covariance: [f64; 9],
    pub linear_acceleration: Vector3,
}

/// `fs_msgs/ControlCommand` — `Header header; float64 throttle, steering, brake`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    pub header: Header,
    pub throttle: f64,
    pub steering: f64,
    pub brake: f64,
}
