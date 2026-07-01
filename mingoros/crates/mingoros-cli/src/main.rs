//! MingoROS CLI — the `mingoros` binary.
//!
//! ROS2 topic debugger for the IFS08 DV stack: MingoCAN, but for ROS topics.
//! The CLI house style follows can-flasher — a `clap` derive tree, a global
//! `--backend` / `--json`, and each subcommand a thin shell over
//! `mingoros-core`.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use mingoros_core::dv_contract::{self, Qos, Reliability};
use mingoros_core::ros::{fake::FakeRos, RosClient};
use std::time::Instant;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "mingoros",
    version,
    about = "MingoROS — ROS2 topic debugger for the IFS08 DV stack (MingoCAN, but for ROS topics)."
)]
struct Cli {
    /// ROS transport backend.
    #[arg(long, value_enum, default_value_t = Backend::Fake, global = true)]
    backend: Backend,

    /// Emit machine-readable JSON instead of human-formatted output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Backend {
    /// In-process synthetic graph — no ROS install required.
    Fake,
    /// Real DDS transport (ros2-client / RustDDS) — not yet built (feat/2).
    Ros2,
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

    /// rosbag record/replay control — planned (dv_msgs StartBag/StopBag).
    Bag,
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Topics => cmd_topics(cli.backend, cli.json),
        Cmd::Echo { topic, count } => cmd_echo(cli.backend, cli.json, &topic, count),
        Cmd::Hz { topic, samples } => cmd_hz(cli.backend, &topic, samples),
        Cmd::Publish {
            topic,
            value,
            force,
        } => cmd_publish(cli.backend, &topic, &value, force),
        Cmd::Bag => cmd_planned("bag", "rosbag record/replay via dv_msgs StartBag/StopBag"),
    }
}

fn make_client(backend: Backend) -> Result<Box<dyn RosClient>> {
    match backend {
        Backend::Fake => Ok(Box::new(FakeRos::new())),
        #[cfg(feature = "ros2")]
        Backend::Ros2 => Ok(Box::new(mingoros_core::ros::ros2::Ros2Client::new()?)),
        #[cfg(not(feature = "ros2"))]
        Backend::Ros2 => Err(mingoros_core::ros::RosError::TransportUnavailable.into()),
    }
}

fn cmd_topics(backend: Backend, json: bool) -> Result<()> {
    let client = make_client(backend)?;
    let mut topics = client.list_topics()?;
    topics.sort_by(|a, b| a.name.cmp(&b.name));

    if json {
        println!("{}", serde_json::to_string_pretty(&topics)?);
        return Ok(());
    }

    eprintln!("backend: {}\n", client.backend_name());
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

fn cmd_echo(backend: Backend, json: bool, topic: &str, count: Option<u64>) -> Result<()> {
    let client = make_client(backend)?;
    let mut stream = client.subscribe(topic)?;
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

fn cmd_hz(backend: Backend, topic: &str, samples: u64) -> Result<()> {
    if samples < 2 {
        bail!("--samples must be >= 2 to compute a rate");
    }
    let client = make_client(backend)?;
    let mut stream = client.subscribe(topic)?;
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

fn cmd_publish(backend: Backend, topic: &str, value: &str, force: bool) -> Result<()> {
    // ROS-side actuation safety gate (mirrors the CAN danger-frame deny-list).
    if is_actuation(topic) && !force {
        bail!(
            "refusing to publish to actuation/command topic {topic} without --force.\n\
             {topic} is a command path (arm it explicitly): re-run with --force if you \
             really intend to drive it."
        );
    }
    let client = make_client(backend)?;
    client.publish(topic, value)?;
    println!("published to {topic}: {value}");
    Ok(())
}

fn cmd_planned(name: &str, what: &str) -> Result<()> {
    println!("`{name}` is planned but not yet wired: {what}.");
    println!("See mingoros/ROADMAP.md.");
    Ok(())
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
