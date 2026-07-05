//! ISC MingoROS CLI — the `mingoros` binary.
//!
//! ROS2 topic debugger for the IFS08 DV stack: MingoCAN, but for ROS topics.
//! The CLI house style follows can-flasher — a `clap` derive tree, a global
//! `--backend` / `--json`, and each subcommand a thin shell over
//! `mingoros-core`.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use mingoros_core::dashboard;
use mingoros_core::dv_contract::{self, Qos, Reliability};
use mingoros_core::ros::{fake::FakeRos, RosClient};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "mingoros",
    version,
    about = "ISC MingoROS — ROS2 topic debugger for the IFS08 DV stack (MingoCAN, but for ROS topics)."
)]
struct Cli {
    /// ROS transport backend.
    #[arg(long, value_enum, default_value_t = Backend::Fake, global = true)]
    backend: Backend,

    /// ROS domain id (must match the DV PC pipeline — the pipeline uses 0).
    #[arg(long, default_value_t = 0, global = true)]
    domain: u16,

    /// Bind DDS to this LOCAL interface IP (your direct-link Ethernet IP), so
    /// discovery goes over the cable to the DV PC instead of WiFi. `ros2`
    /// backend only. RustDDS has no remote-peer knob — this is the local NIC.
    #[arg(long, value_name = "IP", global = true)]
    iface: Option<IpAddr>,

    /// Emit machine-readable JSON instead of human-formatted output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

/// The connection knobs threaded to every command that opens a client.
#[derive(Copy, Clone)]
struct Conn {
    backend: Backend,
    domain: u16,
    iface: Option<IpAddr>,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Backend {
    /// In-process synthetic graph — no ROS install required.
    Fake,
    /// Real DDS transport (ros2-client / RustDDS) — not yet built (feat/2).
    Ros2,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum EbsState {
    /// Engage the EBS — fire the emergency brake (needs --force).
    On,
    /// Return the EBS to normal.
    Off,
}

#[derive(Subcommand)]
enum Cmd {
    /// List topics visible on the graph (name, type, QoS).
    #[command(visible_alias = "ls")]
    Topics,

    /// Subscribe to a topic and print messages (Ctrl-C, or --count, to stop).
    Echo {
        /// Topic name, e.g. /dv/status
        topic: String,
        /// Stop after this many messages (default: run until Ctrl-C).
        #[arg(long)]
        count: Option<u64>,
    },

    /// Measure a topic's publish rate.
    Hz {
        /// Topic name, e.g. /imu/data_raw
        topic: String,
        /// Number of samples to average over.
        #[arg(long, default_value_t = 10)]
        samples: u64,
    },

    /// Publish a value onto a topic. Command/actuation topics require --force
    /// (the ROS-side safety gate).
    #[command(name = "pub")]
    Publish {
        /// Topic name, e.g. /ami/mission
        topic: String,
        /// Value to publish (backend type-checks it; the fake logs it).
        value: String,
        /// Arm an actuation/command topic (disarmed by default).
        #[arg(long)]
        force: bool,
    },

    /// Live safety/state dashboard — subscribes the priority topics
    /// (AS state, DV status, RES, mission, /debug) and renders one panel.
    /// The view for commissioning a stopped car.
    State {
        /// Stop after this many seconds (default: run until Ctrl-C).
        #[arg(long)]
        duration: Option<u64>,
    },

    /// Detect the uDV on the system's USB/serial ports (ranked candidates).
    Udv,

    /// List this host's network interfaces + IPs — pick the direct-link
    /// Ethernet's IP to pass as `--iface` when connecting to the DV PC.
    Ifaces,

    /// Emit the DV contract (topic names, types, QoS, enums) as source the
    /// pipeline can import — a single source of truth so topic-name / QoS drift
    /// can't silently break DDS matching. Python by default; `--json` for a
    /// machine dict.
    Codegen,

    /// Lint the live graph against the contract: type drift on a contract
    /// topic, missing contract topics, and unknown topics. Exits non-zero on a
    /// TYPE mismatch (usable as a CI gate). Needs `--backend ros2` for a live
    /// graph.
    Doctor,

    /// Run a declarative commissioning spec (JSON) as a pass/fail interlock
    /// sequence against the live graph — the scripted pre-run checklist for a
    /// stopped car. Exits non-zero on any FAIL. Each check: {topic, one of
    /// contains/equals/present}.
    Commission {
        /// Path to the JSON spec.
        spec: String,
    },

    /// Capture a topic's decoded (dv_contract-typed) samples to a columnar CSV
    /// for analysis — import straight into pandas / DuckDB (and `COPY … TO
    /// 'x.parquet'` for Parquet). Needs `--backend ros2` for live data.
    Export {
        /// Topic to capture, e.g. /debug
        topic: String,
        /// Output CSV path.
        #[arg(long, default_value = "mingoros_export.csv")]
        out: String,
        /// Stop after this many rows (default: run until Ctrl-C).
        #[arg(long)]
        count: Option<u64>,
    },

    /// Bridge a uDV onto the ROS graph via `micro_ros_agent` (so `--backend
    /// ros2` can see it). Auto-detects the uDV unless --dev is given.
    Agent {
        /// Serial device, e.g. /dev/ttyACM0 (default: auto-detect the uDV).
        #[arg(long)]
        dev: Option<String>,
        /// Serial baud (nominal for USB-CDC).
        #[arg(long, default_value_t = 115_200)]
        baud: u32,
    },

    /// Record or replay a bench session (wraps `ros2 bag`).
    Bag {
        #[command(subcommand)]
        action: BagCmd,
    },

    /// Trigger the uDV's force-EBS service (std_srvs/SetBool) — engage the
    /// Emergency Brake System for a car-on-stands checkup, then release it.
    /// `on` requires --force (it fires the emergency brake).
    #[command(name = "force-ebs")]
    ForceEbs {
        /// `on` engages the EBS (needs --force); `off` returns it to normal.
        #[arg(value_enum)]
        state: EbsState,
        /// Arm the actuation — required for `on`.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum BagCmd {
    /// Record topics to an MCAP bag (wraps `ros2 bag record`).
    Record {
        /// Output directory for the bag.
        #[arg(long, default_value = "mingoros_bag")]
        output: String,
        /// Record every topic (default: just the priority safety topics).
        #[arg(long)]
        all: bool,
    },
    /// Replay a recorded bag (wraps `ros2 bag play`) — e.g. drive the pipeline
    /// through a recorded cone track for an end-to-end "moving car" test.
    Play {
        /// Path to the bag directory.
        path: String,
        /// Loop the bag forever (continuous E2E — Ctrl-C to stop).
        #[arg(long = "loop")]
        loop_: bool,
        /// Playback rate multiplier (e.g. 2.0 = 2×, 0.5 = half speed).
        #[arg(long)]
        rate: Option<f64>,
    },
    /// List recorded bags under a directory (the bag library) — pure-Rust, no
    /// ROS needed. Shows name, size, and file count per bag.
    List {
        /// Directory to scan for bags.
        #[arg(long, default_value = ".")]
        dir: String,
    },
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let conn = Conn {
        backend: cli.backend,
        domain: cli.domain,
        iface: cli.iface,
    };
    match cli.cmd {
        Cmd::Topics => cmd_topics(conn, cli.json),
        Cmd::Echo { topic, count } => cmd_echo(conn, cli.json, &topic, count),
        Cmd::Hz { topic, samples } => cmd_hz(conn, &topic, samples),
        Cmd::Publish {
            topic,
            value,
            force,
        } => cmd_publish(conn, &topic, &value, force),
        Cmd::State { duration } => cmd_state(conn, cli.json, duration),
        Cmd::Udv => cmd_udv(cli.json),
        Cmd::Ifaces => cmd_ifaces(cli.json),
        Cmd::Codegen => cmd_codegen(cli.json),
        Cmd::Doctor => cmd_doctor(conn, cli.json),
        Cmd::Commission { spec } => cmd_commission(conn, cli.json, &spec),
        Cmd::Export { topic, out, count } => cmd_export(conn, &topic, &out, count),
        Cmd::Agent { dev, baud } => cmd_agent(dev, baud),
        Cmd::Bag { action } => cmd_bag(action, cli.json),
        Cmd::ForceEbs { state, force } => cmd_force_ebs(conn, state, force),
    }
}

fn make_client(conn: Conn) -> Result<Box<dyn RosClient>> {
    match conn.backend {
        Backend::Fake => Ok(Box::new(FakeRos::new())),
        #[cfg(feature = "ros2")]
        Backend::Ros2 => Ok(Box::new(mingoros_core::ros::ros2::Ros2Client::new(
            conn.domain,
            conn.iface,
        )?)),
        #[cfg(not(feature = "ros2"))]
        Backend::Ros2 => Err(mingoros_core::ros::RosError::TransportUnavailable.into()),
    }
}

fn cmd_topics(conn: Conn, json: bool) -> Result<()> {
    let client = make_client(conn)?;
    let mut topics = client.list_topics()?;
    topics.sort_by(|a, b| a.name.cmp(&b.name));

    if json {
        println!("{}", serde_json::to_string_pretty(&topics)?);
        return Ok(());
    }

    let ctx = conn
        .iface
        .map(|i| format!("  iface: {i}"))
        .unwrap_or_default();
    eprintln!(
        "backend: {}  domain: {}{ctx}\n",
        client.backend_name(),
        conn.domain
    );
    println!("{:<16} {:<38} {:<24} NOTE", "TOPIC", "TYPE", "QOS");
    for t in &topics {
        println!(
            "{:<16} {:<38} {:<24} {}",
            t.name,
            t.type_name,
            fmt_qos(t.qos.as_ref()),
            t.note.as_deref().unwrap_or("")
        );
    }
    println!("\n{} topics", topics.len());
    Ok(())
}

fn cmd_echo(conn: Conn, json: bool, topic: &str, count: Option<u64>) -> Result<()> {
    let client = make_client(conn)?;
    // Generic path: decode any topic (contract topics richly, standard ROS
    // types by their type name, unknown types as liveness) rather than only
    // the DV-contract set.
    let mut stream = client.subscribe_raw(topic)?;
    if !json {
        eprintln!("echoing {topic} (Ctrl-C to stop)\n");
    }
    let mut n = 0u64;
    while count.map(|c| n < c).unwrap_or(true) {
        match stream.next_sample() {
            Some(s) => {
                if json {
                    println!("{}", serde_json::to_string(&s)?);
                } else {
                    println!("[{:>5}] t+{:>6}ms  {}", s.seq, s.t_ms, s.summary);
                }
                n += 1;
            }
            None => break,
        }
    }
    Ok(())
}

fn cmd_hz(conn: Conn, topic: &str, samples: u64) -> Result<()> {
    if samples < 2 {
        bail!("--samples must be >= 2 to compute a rate");
    }
    let client = make_client(conn)?;
    let mut stream = client.subscribe_raw(topic)?;
    eprintln!("measuring {topic} over {samples} samples...");

    let start = Instant::now();
    let mut stamps = Vec::with_capacity(samples as usize);
    for _ in 0..samples {
        match stream.next_sample() {
            Some(_) => stamps.push(start.elapsed()),
            None => break,
        }
    }
    if stamps.len() < 2 {
        bail!(
            "stream ended before {samples} samples (got {})",
            stamps.len()
        );
    }
    let span = (*stamps.last().unwrap() - stamps[0]).as_secs_f64();
    let rate = (stamps.len() as f64 - 1.0) / span;

    // min/max inter-arrival, useful for spotting jitter.
    let mut min_dt = f64::MAX;
    let mut max_dt = 0.0f64;
    for w in stamps.windows(2) {
        let dt = (w[1] - w[0]).as_secs_f64();
        min_dt = min_dt.min(dt);
        max_dt = max_dt.max(dt);
    }
    println!(
        "average rate: {rate:.2} Hz  (n={}, window={:.3}s, min={:.1}ms max={:.1}ms)",
        stamps.len(),
        span,
        min_dt * 1e3,
        max_dt * 1e3
    );
    if let Some(spec) = dv_contract::KNOWN_TOPICS.iter().find(|s| s.name == topic) {
        // Nudge if a byte heartbeat is running below the contract minimum.
        let is_heartbeat =
            topic == dv_contract::TOPIC_ASSI_STATE || topic == dv_contract::TOPIC_DV_STATUS;
        if is_heartbeat && rate < dv_contract::MIN_HEARTBEAT_HZ {
            println!(
                "  ⚠ below the {:.1} Hz heartbeat minimum for {} — reconciler would treat this as stale",
                dv_contract::MIN_HEARTBEAT_HZ, spec.name
            );
        }
    }
    Ok(())
}

fn cmd_state(conn: Conn, json: bool, duration: Option<u64>) -> Result<()> {
    let client = make_client(conn)?;
    let backend_name = client.backend_name();
    let snap: dashboard::Snapshot = Arc::new(Mutex::new(HashMap::new()));
    let unavailable = dashboard::spawn_subscribers(client.as_ref(), &snap);

    let start = Instant::now();
    loop {
        render_state(backend_name, &snap, &unavailable, json);
        std::thread::sleep(Duration::from_millis(250));
        if let Some(d) = duration {
            if start.elapsed().as_secs() >= d {
                break;
            }
        }
    }
    if !json {
        println!();
    }
    drop(client); // keep the DDS node alive until the RX threads are done with it
    Ok(())
}

fn render_state(
    backend: &str,
    snap: &dashboard::Snapshot,
    unavailable: &[&'static str],
    json: bool,
) {
    use std::io::{IsTerminal, Write};

    if json {
        let rows = dashboard::snapshot(snap, unavailable);
        println!("{}", serde_json::json!({ "topics": rows }));
        return;
    }

    let g = snap.lock().unwrap();

    let tty = std::io::stdout().is_terminal();
    let dot = |code: &str| {
        if tty {
            format!("\x1b[{code}m●\x1b[0m")
        } else {
            "*".to_string()
        }
    };
    let dim = |s: &str| {
        if tty {
            format!("\x1b[31m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };

    let stale = Duration::from_secs_f64(dv_contract::STALENESS_WATCHDOG_S);
    let mut out = String::new();
    if tty {
        out.push_str("\x1b[H\x1b[J"); // cursor home + clear to end
    }
    out.push_str(&format!(
        "ISC MingoROS · DV state   backend:{backend}   (Ctrl-C to exit)\n"
    ));
    out.push_str(&"─".repeat(76));
    out.push('\n');
    for &(label, topic) in dashboard::STATE_TOPICS {
        let (marker, value, age) = match g.get(topic) {
            Some(e) => {
                let secs = e.last.elapsed().as_secs_f64();
                if e.last.elapsed() > stale {
                    (
                        dot("31"),
                        e.summary.clone(),
                        dim(&format!("{secs:.1}s stale")),
                    )
                } else {
                    (dot("32"), e.summary.clone(), format!("{secs:.1}s"))
                }
            }
            None if unavailable.contains(&topic) => (
                dot("90"),
                "(unavailable on this backend)".to_string(),
                String::new(),
            ),
            None => (dot("90"), "(waiting…)".to_string(), String::new()),
        };
        // Highlight danger states (EMERGENCY / ESTOP / EBS active / FAILED) in
        // bold red — the glance-and-see signal for commissioning.
        let value = if tty && dashboard::is_danger(&value) {
            format!("\x1b[1;31m{value}\x1b[0m")
        } else {
            value
        };
        out.push_str(&format!(
            "  {marker} {label:<9} {topic:<13} {value}  {age}\n"
        ));
    }
    out.push_str(&"─".repeat(76));
    out.push('\n');
    print!("{out}");
    let _ = std::io::stdout().flush();
}

fn cmd_udv(json: bool) -> Result<()> {
    let found = mingoros_core::agent::detect_udv().map_err(|e| anyhow::anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&found)?);
        return Ok(());
    }
    if found.is_empty() {
        println!("No uDV candidate found.");
        println!("Plug in the board (enumerates as an ST CDC-ACM, VID:PID 0483:5740) and retry.");
        return Ok(());
    }
    println!("Detected uDV candidate(s) — best first:\n");
    println!("{:<22} {:<5} {:<22} WHY", "PORT", "SCORE", "PRODUCT");
    for m in &found {
        println!(
            "{:<22} {:<5} {:<22} {}",
            m.port,
            m.score,
            m.product.as_deref().unwrap_or("-"),
            m.why
        );
    }
    println!("\nBridge it with:  mingoros agent --dev {}", found[0].port);
    Ok(())
}

fn cmd_ifaces(json: bool) -> Result<()> {
    let ifs = mingoros_core::net::list_interfaces();
    if json {
        println!("{}", serde_json::to_string_pretty(&ifs)?);
        return Ok(());
    }
    if ifs.is_empty() {
        println!("No network interfaces found.");
        return Ok(());
    }
    println!("{:<18} {:<18} KIND", "INTERFACE", "IPv4");
    for i in &ifs {
        println!(
            "{:<18} {:<18} {}",
            i.name,
            i.ip,
            if i.loopback { "loopback" } else { "" }
        );
    }
    let hint = ifs
        .iter()
        .find(|i| !i.loopback)
        .map(|i| i.ip.as_str())
        .unwrap_or("10.42.0.2");
    println!(
        "\nBind DDS to your direct-link Ethernet, e.g.:\n  \
         mingoros topics --backend ros2 --domain 0 --iface {hint}"
    );
    Ok(())
}

/// Emit the DV contract as importable source (Python by default) — the single
/// source of truth for topic names, types, QoS, and the state enums, so the
/// pipeline can't drift out of lockstep with the firmware/app contract.
fn cmd_codegen(json: bool) -> Result<()> {
    use mingoros_core::dv_contract as c;

    let rel = |q: &c::Qos| match q.reliability {
        c::Reliability::Reliable => "reliable",
        c::Reliability::BestEffort => "best_effort",
    };
    let dur = |q: &c::Qos| match q.durability {
        c::Durability::Volatile => "volatile",
        c::Durability::TransientLocal => "transient_local",
    };
    let dir = |d: c::Direction| match d {
        c::Direction::Uplink => "uplink",
        c::Direction::Downlink => "downlink",
        c::Direction::Observe => "observe",
    };
    // "/assi/state" -> "ASSI_STATE"; enum label -> a safe Python constant name.
    let topic_ident = |name: &str| {
        name.trim_start_matches('/')
            .replace(['/', '-', '.'], "_")
            .to_uppercase()
    };
    let member = |label: &str| {
        label
            .chars()
            .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
            .collect::<String>()
            .to_uppercase()
    };

    if json {
        let topics: Vec<_> = c::KNOWN_TOPICS
            .iter()
            .map(|t| {
                let q = c::recommended_qos(t.name);
                serde_json::json!({
                    "name": t.name, "type": t.type_name, "direction": dir(t.direction),
                    "reliability": q.map(|q| rel(&q)), "durability": q.map(|q| dur(&q)),
                    "note": t.note,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "min_heartbeat_hz": c::MIN_HEARTBEAT_HZ,
                "staleness_watchdog_s": c::STALENESS_WATCHDOG_S,
                "topics": topics,
            }))?
        );
        return Ok(());
    }

    let mut o = String::new();
    o.push_str("# Auto-generated by `mingoros codegen` from the ISC MingoROS dv_contract.\n");
    o.push_str(
        "# DO NOT EDIT — regenerate to stay in lockstep with the firmware/app contract.\n\n",
    );
    o.push_str(&format!("MIN_HEARTBEAT_HZ = {}\n", c::MIN_HEARTBEAT_HZ));
    o.push_str(&format!(
        "STALENESS_WATCHDOG_S = {}\n\n",
        c::STALENESS_WATCHDOG_S
    ));

    o.push_str("# --- Topic name constants ---\n");
    for t in c::KNOWN_TOPICS {
        o.push_str(&format!(
            "{} = {:?}  # {}\n",
            topic_ident(t.name),
            t.name,
            t.type_name
        ));
    }

    o.push_str("\n# --- Topic registry (name -> spec) ---\nTOPICS = {\n");
    for t in c::KNOWN_TOPICS {
        let q = c::recommended_qos(t.name);
        o.push_str(&format!(
            "    {:?}: {{\"type\": {:?}, \"reliability\": {:?}, \"durability\": {:?}, \"direction\": {:?}}},\n",
            t.name, t.type_name,
            q.map(|q| rel(&q)).unwrap_or("default"),
            q.map(|q| dur(&q)).unwrap_or("default"),
            dir(t.direction),
        ));
    }
    o.push_str("}\n\n# --- State enums ---\n");

    // Probe the from_* constructors over a safe range to recover (label, value).
    o.push_str("class AsState:\n");
    for v in 0u8..=8 {
        if let Some(s) = c::AsState::from_u8(v) {
            o.push_str(&format!("    {} = {}\n", member(s.label()), v));
        }
    }
    o.push_str("\nclass RawAsState:\n");
    for v in 0u8..=8 {
        if let Some(s) = c::RawAsState::from_u8(v) {
            o.push_str(&format!("    {} = {}\n", member(s.label()), v));
        }
    }
    o.push_str("\nclass DvStatus:\n");
    for v in 0u8..=8 {
        if let Some(s) = c::DvStatus::from_u8(v) {
            o.push_str(&format!("    {} = {}\n", member(s.label()), v));
        }
    }
    o.push_str("\nclass ResStatus:\n");
    for v in -3i32..=5 {
        if let Some(s) = c::ResStatus::from_i32(v) {
            o.push_str(&format!("    {} = {}\n", member(s.label()), v));
        }
    }
    o.push_str("\nclass Mission:\n");
    for v in 0u8..=9 {
        if let Some(s) = c::Mission::from_id(v) {
            o.push_str(&format!("    {} = {}\n", member(s.name()), v));
        }
    }

    print!("{o}");
    Ok(())
}

/// Lint the live graph against the contract — type drift, missing contract
/// topics, unknown topics. Exits non-zero on a TYPE mismatch (CI gate).
fn cmd_doctor(conn: Conn, json: bool) -> Result<()> {
    use mingoros_core::dv_contract as c;
    let client = make_client(conn)?;
    let discovered = client.list_topics()?;
    let by_name: HashMap<&str, &mingoros_core::ros::TopicInfo> =
        discovered.iter().map(|t| (t.name.as_str(), t)).collect();
    let known: std::collections::HashSet<&str> = c::KNOWN_TOPICS.iter().map(|s| s.name).collect();
    let is_builtin = |n: &str| {
        n.contains("mingoros")
            || n.contains("parameter_events")
            || n.contains("rosout")
            || n.ends_with("/clock")
    };

    // (severity, topic, detail)
    let mut rows: Vec<(&'static str, String, String)> = Vec::new();
    for spec in c::KNOWN_TOPICS {
        match by_name.get(spec.name) {
            None => rows.push((
                "missing",
                spec.name.to_string(),
                format!("expected {} — not on the graph", spec.type_name),
            )),
            Some(t) if !t.type_name.is_empty() && t.type_name != spec.type_name => rows.push((
                "type",
                spec.name.to_string(),
                format!(
                    "graph has {}, contract expects {}",
                    t.type_name, spec.type_name
                ),
            )),
            Some(_) => {}
        }
    }
    for t in &discovered {
        if !known.contains(t.name.as_str()) && !is_builtin(&t.name) {
            rows.push((
                "unknown",
                t.name.clone(),
                format!("{} — not in the contract", t.type_name),
            ));
        }
    }

    let count = |s: &str| rows.iter().filter(|(sev, _, _)| *sev == s).count();
    let type_errs = count("type");

    if json {
        let findings: Vec<_> = rows
            .iter()
            .map(|(s, t, d)| serde_json::json!({ "severity": s, "topic": t, "detail": d }))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "discovered": discovered.len(),
                "type_errors": type_errs,
                "findings": findings,
            }))?
        );
    } else if rows.is_empty() {
        println!(
            "✓ contract clean — {} topics on the graph, no type drift.",
            discovered.len()
        );
    } else {
        for (sev, topic, detail) in &rows {
            let mark = match *sev {
                "type" => "✗ TYPE  ",
                "missing" => "· missing",
                _ => "? unknown",
            };
            println!("{mark}  {topic}  —  {detail}");
        }
        println!(
            "\n{type_errs} type mismatch(es) · {} missing · {} unknown  ({} topics on graph)",
            count("missing"),
            count("unknown"),
            discovered.len()
        );
    }
    if type_errs > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Evaluate one commissioning check against the observed topic value (the
/// decoded sample summary, or `None` if the topic produced no sample).
fn eval_check(chk: &serde_json::Value, observed: Option<&str>) -> (bool, String) {
    if let Some(want) = chk.get("contains").and_then(|v| v.as_str()) {
        return (
            observed.is_some_and(|o| o.contains(want)),
            format!("contains '{want}'"),
        );
    }
    if let Some(want) = chk.get("equals").and_then(|v| v.as_str()) {
        return (observed == Some(want), format!("equals '{want}'"));
    }
    if chk.get("present").and_then(|v| v.as_bool()) == Some(true) {
        return (observed.is_some(), "present".to_string());
    }
    (
        false,
        "invalid check (need contains / equals / present)".to_string(),
    )
}

/// Run a declarative commissioning spec: for each check, observe one sample of
/// its topic and PASS/FAIL it. Exits non-zero on any FAIL (a bench gate).
fn cmd_commission(conn: Conn, json_out: bool, path: &str) -> Result<()> {
    let raw = std::fs::read_to_string(path)?;
    let spec: serde_json::Value = serde_json::from_str(&raw)?;
    let name = spec
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("commission");
    let checks = match spec.get("checks").and_then(|v| v.as_array()) {
        Some(c) => c,
        None => bail!("spec needs a 'checks' array"),
    };
    let client = make_client(conn)?;

    // (topic, pass, expectation, observed)
    let mut rows: Vec<(String, bool, String, String)> = Vec::new();
    for chk in checks {
        let topic = chk
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let observed = client
            .subscribe_raw(&topic)
            .ok()
            .and_then(|mut s| s.next_sample())
            .map(|s| s.summary);
        let (pass, expect) = eval_check(chk, observed.as_deref());
        rows.push((
            topic,
            pass,
            expect,
            observed.unwrap_or_else(|| "(no data)".to_string()),
        ));
    }

    let failed = rows.iter().filter(|(_, p, _, _)| !p).count();
    if json_out {
        let arr: Vec<_> = rows
            .iter()
            .map(|(t, p, e, o)| {
                serde_json::json!({ "topic": t, "pass": p, "expect": e, "observed": o })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "name": name, "passed": rows.len() - failed, "failed": failed, "checks": arr,
            }))?
        );
    } else {
        println!("commission: {name}\n");
        for (t, p, e, o) in &rows {
            println!(
                "  {}  {:<22} {:<22} {}",
                if *p { "PASS" } else { "FAIL" },
                t,
                e,
                o
            );
        }
        println!(
            "\n{}/{} checks passed{}",
            rows.len() - failed,
            rows.len(),
            if failed > 0 {
                " — FAIL"
            } else {
                " — all clear"
            }
        );
    }
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// CSV-escape a field: quote + double inner quotes when it has a comma/quote/nl.
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Capture a topic's decoded samples to a columnar CSV (t_ms, seq, topic,
/// value) — the dv_contract-typed → columnar exporter. Long/tidy format so it
/// loads straight into pandas / DuckDB (→ Parquet is one DuckDB COPY away).
fn cmd_export(conn: Conn, topic: &str, out: &str, count: Option<u64>) -> Result<()> {
    use std::io::Write;
    let client = make_client(conn)?;
    let mut stream = client.subscribe_raw(topic)?;
    let mut f = std::io::BufWriter::new(std::fs::File::create(out)?);
    writeln!(f, "t_ms,seq,topic,value")?;
    eprintln!("exporting {topic} → {out} (Ctrl-C to stop)");
    let mut n = 0u64;
    while count.is_none_or(|c| n < c) {
        match stream.next_sample() {
            Some(s) => {
                writeln!(
                    f,
                    "{},{},{},{}",
                    s.t_ms,
                    s.seq,
                    csv_field(&s.topic),
                    csv_field(&s.summary)
                )?;
                n += 1;
            }
            None => break,
        }
    }
    f.flush()?;
    eprintln!("wrote {n} rows to {out}");
    eprintln!("→ DuckDB Parquet:  duckdb -c \"COPY (FROM '{out}') TO 'out.parquet'\"");
    Ok(())
}

fn cmd_agent(dev: Option<String>, baud: u32) -> Result<()> {
    use mingoros_core::agent::{detect_udv, micro_ros_agent_argv, AgentConfig, AgentTransport};

    let dev = match dev {
        Some(d) => d,
        None => {
            let found = detect_udv().map_err(|e| anyhow::anyhow!("{e}"))?;
            match found.into_iter().next() {
                Some(m) => {
                    eprintln!("auto-detected uDV at {} (score {}; {})", m.port, m.score, m.why);
                    m.port
                }
                None => bail!(
                    "no uDV detected — plug in the board or pass --dev /dev/ttyACMx (see `mingoros udv`)"
                ),
            }
        }
    };

    let cfg = AgentConfig {
        transport: AgentTransport::Serial,
        dev,
        baud,
        verbose: 4,
    };
    let argv = micro_ros_agent_argv(&cfg);
    eprintln!("starting: micro_ros_agent {}", argv.join(" "));
    eprintln!(
        "(the uDV runs a ~10 s gyro-cal at boot — keep it still; its topics appear once the \
         XRCE session establishes. Ctrl-C to stop.)\n"
    );

    match std::process::Command::new("micro_ros_agent")
        .args(&argv)
        .status()
    {
        Ok(s) => {
            eprintln!("\nmicro_ros_agent exited ({s}).");
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => bail!(
            "`micro_ros_agent` not found on PATH. It's a ROS 2 / micro-ROS package — install it \
             (or run ISC MingoROS in the container where it lives) and ensure it's on PATH."
        ),
        Err(e) => bail!("failed to launch micro_ros_agent: {e}"),
    }
}

fn cmd_publish(conn: Conn, topic: &str, value: &str, force: bool) -> Result<()> {
    // ROS-side actuation safety gate (mirrors the CAN danger-frame deny-list).
    if is_actuation(topic) && !force {
        bail!(
            "refusing to publish to actuation/command topic {topic} without --force.\n\
             {topic} is a command path (arm it explicitly): re-run with --force if you \
             really intend to drive it."
        );
    }
    let client = make_client(conn)?;
    client.publish(topic, value)?;
    println!("published to {topic}: {value}");
    Ok(())
}

fn cmd_force_ebs(conn: Conn, state: EbsState, force: bool) -> Result<()> {
    let engage = matches!(state, EbsState::On);
    // Actuation safety gate: engaging the EBS must be armed explicitly.
    if engage && !force {
        bail!(
            "refusing to ENGAGE the emergency brake without --force.\n\
             `force-ebs on` fires the EBS actuators via {} — only do this with the car \
             jacked up / on stands. Re-run:  mingoros force-ebs on --force",
            dv_contract::SERVICE_FORCE_EBS
        );
    }
    let client = make_client(conn)?;
    eprintln!(
        "{} the EBS via {} (backend: {}) …",
        if engage { "ENGAGING" } else { "releasing" },
        dv_contract::SERVICE_FORCE_EBS,
        client.backend_name()
    );
    let out = client.call_set_bool(dv_contract::SERVICE_FORCE_EBS, engage)?;
    let msg = if out.message.is_empty() {
        "(no message)"
    } else {
        &out.message
    };
    if out.success {
        println!(
            "{} — uDV: {msg}",
            if engage {
                "EBS ENGAGED"
            } else {
                "EBS released (normal)"
            }
        );
        Ok(())
    } else {
        bail!("uDV reported the service FAILED — {msg}");
    }
}

/// Human-readable byte size (B / KB / MB / GB).
fn human_bytes(b: u64) -> String {
    const U: [&str; 4] = ["B", "KB", "MB", "GB"];
    let (mut f, mut i) = (b as f64, 0usize);
    while f >= 1024.0 && i < 3 {
        f /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{f:.1} {}", U[i])
    }
}

/// The bag library (#47): list recorded bags under a directory. Pure-Rust — a
/// bag is a dir with a `metadata.yaml` and/or `.mcap`/`.db3` files; no ROS.
fn bag_list(dir: &str, json: bool) -> Result<()> {
    let mut bags: Vec<(String, u64, usize)> = Vec::new();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let has_meta = path.join("metadata.yaml").exists();
        let mcaps: Vec<_> = std::fs::read_dir(&path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                matches!(
                    p.extension().and_then(|x| x.to_str()),
                    Some("mcap") | Some("db3")
                )
            })
            .collect();
        if !has_meta && mcaps.is_empty() {
            continue;
        }
        let size: u64 = mcaps
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into();
        bags.push((name, size, mcaps.len()));
    }
    bags.sort();

    if json {
        let arr: Vec<_> = bags
            .iter()
            .map(|(n, s, c)| serde_json::json!({ "name": n, "bytes": s, "files": c }))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "dir": dir, "bags": arr }))?
        );
    } else if bags.is_empty() {
        println!("No bags found under {dir}/.");
    } else {
        println!("{:<34} {:>10}  FILES", "BAG", "SIZE");
        for (n, s, c) in &bags {
            println!("{:<34} {:>10}  {}", n, human_bytes(*s), c);
        }
        println!(
            "\n{} bag(s) in {dir}/. Replay one:  mingoros bag play {dir}/<bag>",
            bags.len()
        );
    }
    Ok(())
}

fn cmd_bag(action: BagCmd, json: bool) -> Result<()> {
    // The library listing is pure-Rust — no ROS needed.
    if let BagCmd::List { dir } = &action {
        return bag_list(dir, json);
    }
    let (argv, note): (Vec<String>, String) = match action {
        BagCmd::Record { output, all } => {
            let mut a = vec![
                "bag".into(),
                "record".into(),
                "-s".into(),
                "mcap".into(),
                "-o".into(),
                output.clone(),
            ];
            if all {
                a.push("-a".into());
            } else {
                // Default: just the priority safety/state topics.
                for &(_, topic) in dashboard::STATE_TOPICS {
                    a.push(topic.to_string());
                }
            }
            (
                a,
                format!("recording to {output}/ — Ctrl-C to stop + finalise"),
            )
        }
        BagCmd::Play { path, loop_, rate } => {
            let mut a = vec!["bag".into(), "play".into(), path.clone()];
            if loop_ {
                a.push("--loop".into());
            }
            if let Some(r) = rate {
                a.push("--rate".into());
                a.push(r.to_string());
            }
            let mut n = format!("replaying {path}");
            if loop_ {
                n.push_str(" (looping — Ctrl-C to stop)");
            }
            (a, n)
        }
        BagCmd::List { .. } => unreachable!("handled above"),
    };

    eprintln!("ros2 {}", argv.join(" "));
    eprintln!("({note})\n");
    match std::process::Command::new("ros2").args(&argv).status() {
        Ok(s) => {
            eprintln!("\nros2 bag exited ({s}).");
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => bail!(
            "`ros2` not found on PATH. Bag record/replay wraps the ROS 2 CLI — run ISC MingoROS \
             where a sourced ROS 2 lives (e.g. the pipeline container)."
        ),
        Err(e) => bail!("failed to launch ros2 bag: {e}"),
    }
}

/// Command/actuation topics that must be explicitly armed before publishing.
fn is_actuation(topic: &str) -> bool {
    matches!(
        topic,
        t if t == dv_contract::TOPIC_CTRL_CMD
            || t == dv_contract::TOPIC_AMI_MISSION
            || t == dv_contract::TOPIC_ASSI_STATE
            || t == dv_contract::SERVICE_FORCE_EBS
            || t == dv_contract::TOPIC_SIM_MISSION
            || t == dv_contract::TOPIC_SIM_INTENT
            || t == dv_contract::TOPIC_SIM_ESTOP
    )
}

fn fmt_qos(qos: Option<&Qos>) -> String {
    match qos {
        None => "-".to_string(),
        Some(q) => {
            let rel = match q.reliability {
                Reliability::Reliable => "REL",
                Reliability::BestEffort => "BE",
            };
            let dur = match q.durability {
                mingoros_core::dv_contract::Durability::Volatile => "VOL",
                mingoros_core::dv_contract::Durability::TransientLocal => "TL",
            };
            format!("{rel}/{dur} d{}", q.depth)
        }
    }
}

#[cfg(test)]
mod tests {
    use mingoros_core::dashboard::is_danger;

    #[test]
    fn danger_states_flagged() {
        assert!(is_danger("data: 1 (AS_EMERGENCY)"));
        assert!(is_danger("data: 6 (DV_FAILED)"));
        assert!(is_danger("data: 1 (ESTOP)"));
        assert!(is_danger(
            "AS AS_DRIVING || ASMS:on TS:on EBS:on ABS:ok ..."
        ));
        // Nominal states are not flagged.
        assert!(!is_danger("data: 2 (AS_READY)"));
        assert!(!is_danger(
            "AS AS_READY || ASMS:on TS:on EBS:off ABS:ok || RES:GO"
        ));
    }
}
