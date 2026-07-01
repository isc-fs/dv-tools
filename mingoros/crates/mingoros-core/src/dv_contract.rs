//! The stock-typed uDV ↔ DV-pipeline interface — the single source of truth,
//! ported faithfully from the pipeline's canonical Python
//! (`mission_control/mission_control/interface_contract.py`) and the
//! `mode_manager` registry / `fs_msgs` definitions.
//!
//! The DV pipeline and the uDV (the micro-ROS endpoint on the car, or
//! `sim_supervisor` emulating it in sim) exchange ONLY standard ROS 2
//! interface types, so nothing here needs custom messages. The byte values
//! below are mirrored in the uDV firmware (C) and in the pipeline (Python) —
//! this module keeps MingoROS in lockstep with both. The `#[cfg(test)]`
//! parity tests at the bottom pin every byte against the Python source so
//! drift is caught at build time.

use serde::Serialize;

// ---------------------------------------------------------------------------
// AS state machine — /assi/state  (std_msgs/UInt8, FS-Rules T14.9 / uDV MMEE)
//
// The uDV — never the pipeline — owns this state machine; mission_control only
// reacts. Published as a >= ~2 Hz liveness heartbeat.
// ---------------------------------------------------------------------------

/// Autonomous-System state byte carried on [`TOPIC_ASSI_STATE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum AsState {
    Off = 0,
    Emergency = 1,
    Ready = 2,
    Driving = 3,
    Finished = 4,
}

impl AsState {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Off),
            1 => Some(Self::Emergency),
            2 => Some(Self::Ready),
            3 => Some(Self::Driving),
            4 => Some(Self::Finished),
            _ => None,
        }
    }

    /// Human-readable label as shown in the FS-rules / uDV LED panel.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "AS_OFF",
            Self::Emergency => "AS_EMERGENCY",
            Self::Ready => "AS_READY",
            Self::Driving => "AS_DRIVING",
            Self::Finished => "AS_FINISHED",
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline lifecycle — /dv/status  (std_msgs/UInt8)
//
// The pipeline's own lifecycle, reported back to the uDV as the prepare/run
// handshake (the stock stand-in for the old SetMission / RuntimeControl
// action results). The uDV gates AS_READY on DV_READY. Latched
// (TRANSIENT_LOCAL) + >= ~2 Hz heartbeat.
// ---------------------------------------------------------------------------

/// Pipeline-lifecycle status byte carried on [`TOPIC_DV_STATUS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum DvStatus {
    /// nothing prepared
    Idle = 0,
    /// configure / JIT in flight (was SetMission feedback)
    Preparing = 1,
    /// prepared OK (was SetMission Result success=true)
    Ready = 2,
    /// activated, emitting /ctrl/cmd
    Running = 3,
    /// mission complete (was RuntimeControl outcome=finished)
    Finished = 4,
    /// pipeline raised EBS (was outcome=emergency)
    Emergency = 5,
    /// prepare/activate error (was Result success=false/error)
    Failed = 6,
}

impl DvStatus {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Idle),
            1 => Some(Self::Preparing),
            2 => Some(Self::Ready),
            3 => Some(Self::Running),
            4 => Some(Self::Finished),
            5 => Some(Self::Emergency),
            6 => Some(Self::Failed),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "DV_IDLE",
            Self::Preparing => "DV_PREPARING",
            Self::Ready => "DV_READY",
            Self::Running => "DV_RUNNING",
            Self::Finished => "DV_FINISHED",
            Self::Emergency => "DV_EMERGENCY",
            Self::Failed => "DV_FAILED",
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline mission registry (mode_manager/mode_registry.py)
// ---------------------------------------------------------------------------

/// A runnable pipeline mission, with its registry `mission_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum Mission {
    Trackdrive = 1,
    Autocross = 2,
    Accel = 3,
    Skidpad = 4,
    Scruti = 5,
}

impl Mission {
    pub const fn mission_id(self) -> u8 {
        self as u8
    }

    pub const fn from_id(id: u8) -> Option<Self> {
        match id {
            1 => Some(Self::Trackdrive),
            2 => Some(Self::Autocross),
            3 => Some(Self::Accel),
            4 => Some(Self::Skidpad),
            5 => Some(Self::Scruti),
            _ => None,
        }
    }

    /// Registry `mode_name` — the string the `ActivateMode` service expects.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Trackdrive => "trackdrive",
            Self::Autocross => "autocross",
            Self::Accel => "accel",
            Self::Skidpad => "skidpad",
            Self::Scruti => "scruti",
        }
    }
}

/// AMI mission INDEX (uDV `ws2812.c` mission_colors) → pipeline registry
/// `mission_id`. `0` = no autonomy mission / tear down. Mirrors
/// `interface_contract.DEFAULT_AMI_TO_MISSION_ID`. AMI 5/6 both map to
/// `scruti` for now (flagged CONFIRM against AMI firmware upstream).
pub const AMI_TO_MISSION_ID: [(u8, u8); 10] = [
    (0, 0), // Manual        → no autonomy mission
    (1, 3), // Acceleration  → accel
    (2, 4), // Skidpad       → skidpad
    (3, 2), // Autocross     → autocross
    (4, 1), // Track drive   → trackdrive
    (5, 5), // EVS/EBS test  → scruti   (CONFIRM)
    (6, 5), // Inspection    → scruti   (CONFIRM)
    (7, 0), // Shutdown      → no mission
    (8, 0), // Aux1          → no mission
    (9, 0), // Aux2          → no mission
];

/// Translate an AMI mission index to a pipeline registry `mission_id`.
///
/// Returns `0` (no autonomy mission / tear down) for any index not in the
/// table — including the non-autonomy AMI slots (Manual, Shutdown, Aux) — so
/// a glitchy `/ami/mission` can never start an unintended run. Never panics.
pub fn ami_index_to_mission_id(ami_index: i32) -> u8 {
    if ami_index < 0 {
        return 0;
    }
    let idx = ami_index as u8;
    AMI_TO_MISSION_ID
        .iter()
        .find_map(|&(ami, mid)| (ami == idx).then_some(mid))
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Sim operator panel — SIM-ONLY (Linux ↔ Linux). The real car has no
// equivalent (the AMI board / RES drive /ami/mission + AS transitions
// directly). Intent bytes mirror `as_state_machine.OperatorIntent`.
// ---------------------------------------------------------------------------

/// Sim operator intent byte on [`TOPIC_SIM_INTENT`] (sim-only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum SimIntent {
    /// disarmed
    Off = 0,
    /// armed / prepare (RES go not pressed)
    Ready = 1,
    /// RES go — run
    Go = 2,
}

// ---------------------------------------------------------------------------
// Cone colours — fs_msgs/msg/Cone.msg
// ---------------------------------------------------------------------------

/// Cone colour enum from `fs_msgs/Cone`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum ConeColor {
    Yellow = 0,
    Blue = 1,
    OrangeBig = 2,
    OrangeSmall = 3,
    Unknown = 4,
}

impl ConeColor {
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Yellow),
            1 => Some(Self::Blue),
            2 => Some(Self::OrangeBig),
            3 => Some(Self::OrangeSmall),
            4 => Some(Self::Unknown),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Topic / service names — single source of truth (never string-literal a
// topic name at a call site; reference these).
// ---------------------------------------------------------------------------

// Runtime stock-typed uDV ↔ mission_control interface.
pub const TOPIC_ASSI_STATE: &str = "/assi/state"; // std_msgs/UInt8   uplink
pub const TOPIC_AMI_MISSION: &str = "/ami/mission"; // std_msgs/Int32   uplink
pub const TOPIC_DV_STATUS: &str = "/dv/status"; // std_msgs/UInt8   downlink (latched)
pub const TOPIC_CTRL_CMD: &str = "/ctrl/cmd"; // geometry_msgs/Twist downlink
pub const SERVICE_FORCE_EBS: &str = "/force_ebs"; // std_srvs/SetBool (service)

// Sim operator panel (sim-only).
pub const TOPIC_SIM_MISSION: &str = "/sim/mission";
pub const TOPIC_SIM_INTENT: &str = "/sim/intent";
pub const TOPIC_SIM_ESTOP: &str = "/sim/estop";

// Car sensor surface (published by the uDV / Hesai on canonical names).
pub const TOPIC_IMU: &str = "/imu/data_raw"; // sensor_msgs/Imu        ~400 Hz
pub const TOPIC_LIDAR: &str = "/lidar_points"; // sensor_msgs/PointCloud2 ~10 Hz
pub const TOPIC_STEERING: &str = "/steering_angle"; // std_msgs/Float* (RAD)
pub const TOPIC_MOTOR_RPM: &str = "/motor_rpm"; // std_msgs/Float* (shaft RPM)

// Autonomy outputs a debugger visualises.
pub const TOPIC_CONES_RAW: &str = "/Conos_raw"; // MarkerArray
pub const TOPIC_CONES_ORANGE: &str = "/Conos_Orange"; // MarkerArray
pub const TOPIC_CONES: &str = "/Conos"; // MarkerArray
pub const TOPIC_CONES_FULL: &str = "/Conos_full"; // MarkerArray
pub const TOPIC_SLAM_POSE: &str = "/slam/pose"; // nav_msgs/Odometry
pub const TOPIC_ODOM: &str = "/odom"; // nav_msgs/Odometry
pub const TOPIC_PATH: &str = "/Path"; // nav_msgs/Path

// ---------------------------------------------------------------------------
// IFSSIM / sim-pipeline surface — the MingoROS ros2 test bed.
//
// IFSSIM vendors an OLDER pipeline than the uDV stock interface above: it has
// NO /dv/status, /assi/state, /ami/mission, /force_ebs. These are what the
// IFSSIM bag replay + live pipeline actually publish (confirmed live via
// `ros2 topic info -v`). Shared with the car: /Conos*, /odom, /Path,
// /slam/pose. Sim-specific below. See [[project_mingoros]] IFSSIM notes.
// ---------------------------------------------------------------------------
pub const TOPIC_SIM_IMU: &str = "/imu"; // sensor_msgs/Imu (car uses /imu/data_raw)
pub const TOPIC_SIM_LIDAR: &str = "/lidar/Lidar1"; // sensor_msgs/PointCloud2
pub const TOPIC_TESTING_TRACK: &str = "/testing_only/track"; // fs_msgs/Track — RELIABLE/TRANSIENT_LOCAL (latched)
pub const TOPIC_TESTING_ODOM: &str = "/testing_only/odom"; // nav_msgs/Odometry (ground truth, best-effort)
pub const TOPIC_CONE_SLAM_GT_ERROR: &str = "/cone_slam/gt_error_m"; // std_msgs/Float32 (SLAM accuracy diag)
pub const TOPIC_CTRL_V_SET: &str = "/control/v_set_mps"; // std_msgs/Float32
pub const TOPIC_CTRL_KAPPA_MAX: &str = "/control/kappa_max_per_m"; // std_msgs/Float32
pub const TOPIC_CTRL_CMD_INTERNAL: &str = "/ctrl/cmd_internal"; // fs_msgs/ControlCommand
pub const TOPIC_CTRL_EMERGENCY: &str = "/ctrl/emergency"; // std_msgs/Bool — latched
pub const TOPIC_SIGNAL_EBS: &str = "/signal/ebs"; // std_msgs/Empty — latched
pub const TOPIC_SIGNAL_GO: &str = "/signal/go"; // fs_msgs/GoSignal
pub const TOPIC_SLAM_FINISHED: &str = "/slam/finished"; // std_msgs/Bool — latched

/// Minimum heartbeat cadence for the byte topics (`/assi/state`,
/// `/dv/status`): each is the other side's liveness signal. A staler stream
/// is treated as a fault by the reconciler.
pub const MIN_HEARTBEAT_HZ: f64 = 2.0;

/// Staleness watchdog window mirrored from the pipeline reconciler.
pub const STALENESS_WATCHDOG_S: f64 = 1.5;

// ---------------------------------------------------------------------------
// QoS — the load-bearing part. Mismatch here is SILENT no-data, so MingoROS's
// ROS transport MUST honour these when it subscribes. Values pinned against
// the pipeline node sources (see the QoS-validation spike, ROADMAP feat/2).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Reliability {
    Reliable,
    BestEffort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Durability {
    Volatile,
    TransientLocal,
}

/// A reduced QoS profile — the three settings that actually break DDS
/// subscription matching for this pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Qos {
    pub reliability: Reliability,
    pub durability: Durability,
    pub depth: u16,
}

impl Qos {
    pub const fn new(reliability: Reliability, durability: Durability, depth: u16) -> Self {
        Self {
            reliability,
            durability,
            depth,
        }
    }

    /// `sensor_data`-style: BEST_EFFORT + VOLATILE, shallow. High-rate feeds
    /// (`/imu`, `/ctrl/cmd`).
    pub const fn sensor(depth: u16) -> Self {
        Self::new(Reliability::BestEffort, Durability::Volatile, depth)
    }

    /// RELIABLE + VOLATILE. Cone MarkerArrays.
    pub const fn reliable(depth: u16) -> Self {
        Self::new(Reliability::Reliable, Durability::Volatile, depth)
    }

    /// RELIABLE + TRANSIENT_LOCAL (latched). Status bytes a late joiner must
    /// still receive the last value of.
    pub const fn latched(depth: u16) -> Self {
        Self::new(Reliability::Reliable, Durability::TransientLocal, depth)
    }
}

/// Recommended QoS for a known DV topic, or `None` if unknown (fall back to
/// the ROS default and log it — an unknown topic's QoS is a discovery task).
pub fn recommended_qos(topic: &str) -> Option<Qos> {
    let q = match topic {
        // --- car (uDV stock) surface ---
        TOPIC_DV_STATUS => Qos::latched(1),
        TOPIC_ASSI_STATE | TOPIC_AMI_MISSION => Qos::reliable(10),
        TOPIC_CTRL_CMD => Qos::sensor(10),
        TOPIC_IMU => Qos::sensor(10),
        TOPIC_LIDAR => Qos::sensor(5),
        TOPIC_CONES_RAW | TOPIC_CONES_ORANGE | TOPIC_CONES | TOPIC_CONES_FULL => Qos::reliable(10),
        TOPIC_SLAM_POSE | TOPIC_ODOM | TOPIC_PATH => Qos::reliable(10),
        // --- IFSSIM / sim surface (confirmed live via `ros2 topic info -v`) ---
        TOPIC_TESTING_TRACK | TOPIC_CTRL_EMERGENCY | TOPIC_SIGNAL_EBS | TOPIC_SLAM_FINISHED => {
            Qos::latched(1)
        }
        TOPIC_CONE_SLAM_GT_ERROR
        | TOPIC_CTRL_V_SET
        | TOPIC_CTRL_KAPPA_MAX
        | TOPIC_CTRL_CMD_INTERNAL
        | TOPIC_SIGNAL_GO => Qos::reliable(10),
        TOPIC_SIM_IMU | TOPIC_SIM_LIDAR | TOPIC_MOTOR_RPM | TOPIC_STEERING | TOPIC_TESTING_ODOM => {
            Qos::sensor(10)
        }
        _ => return None,
    };
    Some(q)
}

// ---------------------------------------------------------------------------
// Known-topic catalogue — drives the fake backend and the `topics` command
// when no live ROS graph is available.
// ---------------------------------------------------------------------------

/// Direction of a topic relative to the DV pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Direction {
    /// uDV/sensors → pipeline (uplink / input)
    Uplink,
    /// pipeline → uDV/actuators (downlink / output)
    Downlink,
    /// internal pipeline output a debugger observes
    Observe,
}

/// A statically-known DV topic: name, ROS type, recommended QoS, direction.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TopicSpec {
    pub name: &'static str,
    pub type_name: &'static str,
    pub direction: Direction,
    pub note: &'static str,
}

impl TopicSpec {
    pub fn qos(&self) -> Option<Qos> {
        recommended_qos(self.name)
    }
}

/// The catalogue of DV topics MingoROS knows about a priori.
pub const KNOWN_TOPICS: &[TopicSpec] = &[
    TopicSpec {
        name: TOPIC_ASSI_STATE,
        type_name: "std_msgs/msg/UInt8",
        direction: Direction::Uplink,
        note: "AS state machine byte (heartbeat >=2 Hz)",
    },
    TopicSpec {
        name: TOPIC_AMI_MISSION,
        type_name: "std_msgs/msg/Int32",
        direction: Direction::Uplink,
        note: "selected AMI mission index 0..9 (raw)",
    },
    TopicSpec {
        name: TOPIC_DV_STATUS,
        type_name: "std_msgs/msg/UInt8",
        direction: Direction::Downlink,
        note: "pipeline lifecycle byte (latched)",
    },
    TopicSpec {
        name: TOPIC_CTRL_CMD,
        type_name: "geometry_msgs/msg/Twist",
        direction: Direction::Downlink,
        note: "linear.x=throttle[-1,1], angular.z=steer[-1,1]",
    },
    TopicSpec {
        name: TOPIC_IMU,
        type_name: "sensor_msgs/msg/Imu",
        direction: Direction::Uplink,
        note: "uDV IMU, ~400 Hz",
    },
    TopicSpec {
        name: TOPIC_LIDAR,
        type_name: "sensor_msgs/msg/PointCloud2",
        direction: Direction::Uplink,
        note: "Hesai LiDAR, ~10 Hz",
    },
    TopicSpec {
        name: TOPIC_STEERING,
        type_name: "std_msgs/msg/Float32",
        direction: Direction::Uplink,
        note: "steering angle, RADIANS (uDV-converted)",
    },
    TopicSpec {
        name: TOPIC_MOTOR_RPM,
        type_name: "std_msgs/msg/Float32",
        direction: Direction::Uplink,
        note: "motor-shaft RPM (uDV reads inverter CAN)",
    },
    TopicSpec {
        name: TOPIC_CONES,
        type_name: "visualization_msgs/msg/MarkerArray",
        direction: Direction::Observe,
        note: "SLAM cone map",
    },
    TopicSpec {
        name: TOPIC_CONES_RAW,
        type_name: "visualization_msgs/msg/MarkerArray",
        direction: Direction::Observe,
        note: "per-frame detected cones",
    },
    TopicSpec {
        name: TOPIC_SLAM_POSE,
        type_name: "nav_msgs/msg/Odometry",
        direction: Direction::Observe,
        note: "SLAM pose estimate",
    },
    TopicSpec {
        name: TOPIC_ODOM,
        type_name: "nav_msgs/msg/Odometry",
        direction: Direction::Observe,
        note: "EKF odometry",
    },
    TopicSpec {
        name: TOPIC_PATH,
        type_name: "nav_msgs/msg/Path",
        direction: Direction::Observe,
        note: "planned path",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    // --- Parity with interface_contract.py byte values ---
    #[test]
    fn as_state_bytes_match_python() {
        assert_eq!(AsState::Off.as_u8(), 0);
        assert_eq!(AsState::Emergency.as_u8(), 1);
        assert_eq!(AsState::Ready.as_u8(), 2);
        assert_eq!(AsState::Driving.as_u8(), 3);
        assert_eq!(AsState::Finished.as_u8(), 4);
    }

    #[test]
    fn dv_status_bytes_match_python() {
        assert_eq!(DvStatus::Idle.as_u8(), 0);
        assert_eq!(DvStatus::Preparing.as_u8(), 1);
        assert_eq!(DvStatus::Ready.as_u8(), 2);
        assert_eq!(DvStatus::Running.as_u8(), 3);
        assert_eq!(DvStatus::Finished.as_u8(), 4);
        assert_eq!(DvStatus::Emergency.as_u8(), 5);
        assert_eq!(DvStatus::Failed.as_u8(), 6);
    }

    #[test]
    fn mission_ids_match_registry() {
        assert_eq!(Mission::Trackdrive.mission_id(), 1);
        assert_eq!(Mission::Autocross.mission_id(), 2);
        assert_eq!(Mission::Accel.mission_id(), 3);
        assert_eq!(Mission::Skidpad.mission_id(), 4);
        assert_eq!(Mission::Scruti.mission_id(), 5);
    }

    #[test]
    fn ami_map_matches_python() {
        // DEFAULT_AMI_TO_MISSION_ID
        assert_eq!(ami_index_to_mission_id(0), 0);
        assert_eq!(ami_index_to_mission_id(1), 3);
        assert_eq!(ami_index_to_mission_id(2), 4);
        assert_eq!(ami_index_to_mission_id(3), 2);
        assert_eq!(ami_index_to_mission_id(4), 1);
        assert_eq!(ami_index_to_mission_id(5), 5);
        assert_eq!(ami_index_to_mission_id(6), 5);
        assert_eq!(ami_index_to_mission_id(7), 0);
        assert_eq!(ami_index_to_mission_id(8), 0);
        assert_eq!(ami_index_to_mission_id(9), 0);
        // Out-of-range / negative never panics, always "no mission".
        assert_eq!(ami_index_to_mission_id(42), 0);
        assert_eq!(ami_index_to_mission_id(-1), 0);
    }

    #[test]
    fn cone_colors_match_msg() {
        assert_eq!(ConeColor::Yellow as u8, 0);
        assert_eq!(ConeColor::Blue as u8, 1);
        assert_eq!(ConeColor::OrangeBig as u8, 2);
        assert_eq!(ConeColor::OrangeSmall as u8, 3);
        assert_eq!(ConeColor::Unknown as u8, 4);
    }

    #[test]
    fn dv_status_is_latched() {
        // The one QoS mistake that silently breaks a late-joining debugger.
        let q = recommended_qos(TOPIC_DV_STATUS).unwrap();
        assert_eq!(q.durability, Durability::TransientLocal);
        assert_eq!(q.reliability, Reliability::Reliable);
    }

    #[test]
    fn roundtrip_from_u8() {
        for b in 0u8..=4 {
            assert_eq!(AsState::from_u8(b).unwrap().as_u8(), b);
        }
        for b in 0u8..=6 {
            assert_eq!(DvStatus::from_u8(b).unwrap().as_u8(), b);
        }
    }
}
