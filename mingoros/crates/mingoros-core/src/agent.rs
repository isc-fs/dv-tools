//! uDV link management — detect the uDV over USB and drive `micro_ros_agent`.
//!
//! The uDV is a micro-ROS (XRCE-DDS) endpoint on USB-CDC; it only appears on
//! the ROS graph once a `micro_ros_agent` bridges its serial link. ISC MingoROS
//! detects the board and owns that agent process, so `--backend ros2` can then
//! see the uDV's topics.
//!
//! Detection caveat: the uDV enumerates as a **generic ST CDC-ACM**
//! (VID:PID `0483:5740`) shared by every default ST USB-CDC board, so VID/PID
//! alone can't uniquely identify it — we rank on the USB product / serial /
//! manufacturer strings too, and let the operator disambiguate with `--dev`.

use serde::Serialize;

/// STMicroelectronics USB vendor id.
pub const ST_VID: u16 = 0x0483;
/// The default ST CDC-ACM product id (generic — NOT unique to the uDV).
pub const CDC_PID: u16 = 0x5740;

/// Case-insensitive substrings that strongly suggest a port is the uDV.
const UDV_HINTS: &[&str] = &["udv", "cubemx", "ifs", "micro-ros", "microros"];

/// USB identity of a serial port (subset of `serialport`'s `UsbPortInfo`).
#[derive(Debug, Clone, Default)]
pub struct UsbId {
    pub vid: u16,
    pub pid: u16,
    pub serial: Option<String>,
    pub product: Option<String>,
    pub manufacturer: Option<String>,
}

/// A serial port considered for uDV detection.
#[derive(Debug, Clone)]
pub struct PortCandidate {
    pub port: String,
    pub usb: Option<UsbId>,
}

/// A ranked uDV match. Higher `score` = more likely the uDV.
#[derive(Debug, Clone, Serialize)]
pub struct UdvMatch {
    pub port: String,
    pub score: u8,
    pub why: String,
    pub serial: Option<String>,
    pub product: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("serial port enumeration failed: {0}")]
    Enumerate(String),
}

/// Rank ports by how likely each is the uDV. Pure — unit-testable with
/// synthetic input. Ports scoring 0 (not even ST CDC) are dropped.
pub fn rank_udv_candidates(ports: &[PortCandidate]) -> Vec<UdvMatch> {
    let mut out: Vec<UdvMatch> = ports
        .iter()
        .filter_map(|p| {
            let usb = p.usb.as_ref()?;
            let mut score = 0u8;
            let mut reasons = Vec::new();

            if usb.vid == ST_VID && usb.pid == CDC_PID {
                score += 2;
                reasons.push("ST CDC 0483:5740".to_string());
            } else if usb.vid == ST_VID {
                score += 1;
                reasons.push("ST vendor".to_string());
            }

            // A name hint is the disambiguator between multiple ST CDC boards.
            let hint = [&usb.product, &usb.manufacturer, &usb.serial]
                .into_iter()
                .flatten()
                .find(|s| {
                    let low = s.to_lowercase();
                    UDV_HINTS.iter().any(|h| low.contains(h))
                });
            if let Some(h) = hint {
                score += 3;
                reasons.push(format!("name hint '{h}'"));
            }

            if score == 0 {
                return None;
            }
            Some(UdvMatch {
                port: p.port.clone(),
                score,
                why: reasons.join(", "),
                serial: usb.serial.clone(),
                product: usb.product.clone(),
            })
        })
        .collect();

    // Best first, then by port name for stable ordering.
    out.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.port.cmp(&b.port)));
    out
}

/// Detect the uDV on the live system's serial ports.
pub fn detect_udv() -> Result<Vec<UdvMatch>, AgentError> {
    let ports = serialport::available_ports().map_err(|e| AgentError::Enumerate(e.to_string()))?;
    let candidates: Vec<PortCandidate> = ports
        .into_iter()
        .map(|p| {
            let usb = match p.port_type {
                serialport::SerialPortType::UsbPort(u) => Some(UsbId {
                    vid: u.vid,
                    pid: u.pid,
                    serial: u.serial_number,
                    product: u.product,
                    manufacturer: u.manufacturer,
                }),
                _ => None,
            };
            PortCandidate {
                port: p.port_name,
                usb,
            }
        })
        .collect();
    Ok(rank_udv_candidates(&candidates))
}

/// `micro_ros_agent` transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTransport {
    /// USB-CDC serial (the uDV's link).
    Serial,
    /// UDPv4 (for a networked micro-ROS endpoint).
    Udp4,
}

/// Configuration for the `micro_ros_agent` subprocess.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub transport: AgentTransport,
    /// Serial device (`/dev/ttyACM0`) or, for UDP, the port number as a string.
    pub dev: String,
    pub baud: u32,
    pub verbose: u8,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            transport: AgentTransport::Serial,
            dev: String::new(),
            baud: 115_200,
            verbose: 4,
        }
    }
}

/// Build the `micro_ros_agent` argv for a config (the program name is added by
/// the caller). Pure + unit-testable.
pub fn micro_ros_agent_argv(cfg: &AgentConfig) -> Vec<String> {
    let mut a = Vec::new();
    match cfg.transport {
        AgentTransport::Serial => {
            a.push("serial".into());
            a.push("--dev".into());
            a.push(cfg.dev.clone());
            a.push("-b".into());
            a.push(cfg.baud.to_string());
        }
        AgentTransport::Udp4 => {
            a.push("udp4".into());
            a.push("--port".into());
            a.push(cfg.dev.clone());
        }
    }
    a.push("-v".into());
    a.push(cfg.verbose.to_string());
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usb(vid: u16, pid: u16, product: Option<&str>, serial: Option<&str>) -> PortCandidate {
        PortCandidate {
            port: format!("/dev/tty.{}", product.unwrap_or("x")),
            usb: Some(UsbId {
                vid,
                pid,
                product: product.map(str::to_string),
                serial: serial.map(str::to_string),
                manufacturer: None,
            }),
        }
    }

    #[test]
    fn named_udv_outranks_generic_st_cdc() {
        let ports = vec![
            usb(ST_VID, CDC_PID, Some("STM32 Virtual ComPort"), None), // generic
            usb(ST_VID, CDC_PID, Some("IFS08 uDV"), Some("udv-01")),   // the real one
            usb(0x2341, 0x0043, Some("Arduino"), None),                // unrelated
        ];
        let ranked = rank_udv_candidates(&ports);
        assert_eq!(ranked.len(), 2, "arduino dropped");
        assert!(ranked[0].product.as_deref() == Some("IFS08 uDV"));
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn generic_st_cdc_still_a_candidate() {
        let ranked = rank_udv_candidates(&[usb(ST_VID, CDC_PID, None, None)]);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].score, 2);
    }

    #[test]
    fn argv_serial_and_udp() {
        let mut c = AgentConfig {
            dev: "/dev/ttyACM0".into(),
            ..Default::default()
        };
        assert_eq!(
            micro_ros_agent_argv(&c),
            ["serial", "--dev", "/dev/ttyACM0", "-b", "115200", "-v", "4"]
        );
        c.transport = AgentTransport::Udp4;
        c.dev = "8888".into();
        assert_eq!(
            micro_ros_agent_argv(&c),
            ["udp4", "--port", "8888", "-v", "4"]
        );
    }
}
