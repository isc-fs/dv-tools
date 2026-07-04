//! Minimal ROS2 message types for the `ros2` backend, as serde structs whose
//! field order matches the `.msg` definitions so RustDDS's CDR (de)serializer
//! reads them correctly off the wire.
//!
//! Deliberately a small subset — the state/safety/motion types ISC MingoROS
//! decodes. (Perception/cones are out of scope.)

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

// --- the rest of the std_msgs scalar family, for the generic echo tab's
// type-keyed decode (a `std_msgs/Xxx` topic on the graph → readable `data:`). ---

/// `std_msgs/Float64` — `float64 data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Float64 {
    pub data: f64,
}

/// `std_msgs/Int8` — `int8 data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Int8 {
    pub data: i8,
}

/// `std_msgs/Int16` — `int16 data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Int16 {
    pub data: i16,
}

/// `std_msgs/Int64` — `int64 data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Int64 {
    pub data: i64,
}

/// `std_msgs/UInt16` — `uint16 data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UInt16 {
    pub data: u16,
}

/// `std_msgs/UInt32` — `uint32 data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UInt32 {
    pub data: u32,
}

/// `std_msgs/UInt64` — `uint64 data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UInt64 {
    pub data: u64,
}

/// `geometry_msgs/Point` — `float64 x, y, z`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
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

/// `geometry_msgs/Accel` — same shape as `Twist` (two `Vector3`s).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Accel {
    pub linear: Vector3,
    pub angular: Vector3,
}

/// `geometry_msgs/PoseStamped`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseStamped {
    pub header: Header,
    pub pose: Pose,
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

/// `sensor_msgs/NavSatStatus` — `int8 status; uint16 service`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavSatStatus {
    pub status: i8,
    pub service: u16,
}

/// `sensor_msgs/NavSatFix` **prefix** — up to `altitude`; the trailing
/// `float64[9] position_covariance` + `uint8 position_covariance_type` unread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavSatFix {
    pub header: Header,
    pub status: NavSatStatus,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}

/// `sensor_msgs/Range` — no trailing arrays, read in full.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub header: Header,
    pub radiation_type: u8,
    pub field_of_view: f32,
    pub min_range: f32,
    pub max_range: f32,
    pub range: f32,
}

/// `sensor_msgs/Temperature` — `Header header; float64 temperature, variance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Temperature {
    pub header: Header,
    pub temperature: f64,
    pub variance: f64,
}

/// `sensor_msgs/FluidPressure` — `Header header; float64 fluid_pressure, variance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FluidPressure {
    pub header: Header,
    pub fluid_pressure: f64,
    pub variance: f64,
}

/// `fs_msgs/ControlCommand` — `Header header; float64 throttle, steering, brake`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    pub header: Header,
    pub throttle: f64,
    pub steering: f64,
    pub brake: f64,
}

// --- service (std_srvs/SetBool) request/response, for the uDV actuation
// services (`/force_ebs`, `/activate_steering`). Field order matches the .srv
// so RustDDS's CDR reads them off the wire correctly. The `ros2_client::Message`
// impls live in `ros/ros2.rs` (that trait only exists under the `ros2` feature).

/// `std_srvs/srv/SetBool` **request** — `bool data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBoolRequest {
    pub data: bool,
}

/// `std_srvs/srv/SetBool` **response** — `bool success`, `string message`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBoolResponse {
    pub success: bool,
    pub message: String,
}
