//! # mingoros-core
//!
//! Core library for **MingoROS** — the DV-stack ROS2 topic debugger. MingoCAN
//! (`isc-fs/can-flasher`) is to CAN frames what MingoROS is to ROS topics:
//! list / echo / hz / publish / record, plus a live cone-map & mission-state
//! dashboard.
//!
//! ## Modules
//! - [`dv_contract`] — the single source of truth for the uDV ↔ pipeline
//!   interface (AS/DV state bytes, mission registry, AMI map, topic + QoS
//!   catalogue), ported faithfully from the pipeline's Python/`fs_msgs`.
//! - [`ros`] — the transport abstraction (`RosClient` trait) + the in-process
//!   [`ros::fake`] backend. The real DDS backend lands behind the `ros2`
//!   feature in a follow-up.

pub mod dv_contract;
pub mod ros;

/// Crate version (`CARGO_PKG_VERSION`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
