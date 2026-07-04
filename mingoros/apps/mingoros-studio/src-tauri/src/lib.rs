//! ISC MingoROS — Tauri command surface + app body.
//!
//! `main.rs` is the binary entry point; the actual app body lives here (mirrors
//! MingoCAN's can-studio) so the window setup + commands are one module and a
//! future integration test can reach `run()` without the `generate_context!`
//! macro (which only runs once per process).
//!
//! Same `mingoros-core` engine as the CLI, in a window: the app joins the car's
//! ROS 2 DDS graph over Ethernet (the `ros2` / RustDDS backend) and serves the
//! Svelte frontend in `../dist`, which calls the commands below via
//! `@tauri-apps/api`'s `invoke`.

use mingoros_core::dashboard::{self, Snapshot};
use mingoros_core::ros::{RosClient, Sample};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Live dashboard state: the snapshot + connection metadata. Locked briefly on
/// every 250 ms `get_state` poll, so nothing slow may hold it.
struct AppState {
    snap: Snapshot,
    unavailable: Vec<&'static str>,
    domain: u16,
    /// Local interface DDS is bound to (the direct-link Ethernet IP), if any.
    iface: Option<IpAddr>,
    /// Topics visible on the graph at connect time — a "is the DV PC reachable"
    /// signal distinct from the live priority-topic count.
    discovered: usize,
    connected: bool,
    error: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            snap: Arc::new(Mutex::new(HashMap::new())),
            unavailable: Vec::new(),
            domain: 0,
            iface: None,
            discovered: 0,
            connected: false,
            error: None,
        }
    }
}

type Shared = Mutex<AppState>;

/// The live DDS client, held **separately** from [`AppState`] and kept alive so
/// the RX threads' subscriptions stay valid. A service call (`force_ebs`) can
/// hold this for a few seconds; keeping it out of `AppState` means that never
/// stalls the dashboard poll, which only touches `AppState`.
type ClientCell = Mutex<Option<Box<dyn RosClient>>>;

/// Build the transport: the real ros2/RustDDS client (on `domain`, optionally
/// bound to a local interface), or the in-process fake (set `MINGOROS_FAKE=1`)
/// for demos / development without a ROS graph.
fn make_client(domain: u16, iface: Option<IpAddr>) -> Result<Box<dyn RosClient>, String> {
    if std::env::var_os("MINGOROS_FAKE").is_some() {
        return Ok(Box::new(mingoros_core::ros::fake::FakeRos::new()));
    }
    Ok(Box::new(
        mingoros_core::ros::ros2::Ros2Client::new(domain, iface).map_err(|e| e.to_string())?,
    ))
}

/// Join (or re-join) the ROS 2 graph on `domain` (optionally bound to `iface`)
/// and start the subscribers.
fn connect_impl(
    domain: u16,
    iface: Option<IpAddr>,
    state: &Shared,
    client_cell: &ClientCell,
    echo: &EchoCell,
) -> Result<(), String> {
    // (Re)joining a graph invalidates any running echo streams — they were
    // bound to the old participant and the topics may not exist on the new
    // graph. Stop them first so nothing keeps pumping from the dropped client.
    clear_echo(echo);
    let client = make_client(domain, iface)?;
    let snap: Snapshot = Arc::new(Mutex::new(HashMap::new()));
    let unavailable = dashboard::spawn_subscribers(client.as_ref(), &snap);
    // How many topics are on the graph right now — 0 usually means the link /
    // multicast isn't reaching the DV PC (vs. topics simply being quiet).
    let discovered = client.list_topics().map(|v| v.len()).unwrap_or(0);
    {
        let mut s = state.lock().unwrap_or_else(|p| p.into_inner());
        s.snap = snap;
        s.unavailable = unavailable;
        s.domain = domain;
        s.iface = iface;
        s.discovered = discovered;
        s.connected = true;
        s.error = None;
    }
    // Kept alive for the RX threads' lifetime; used by `force_ebs`.
    *client_cell.lock().unwrap_or_else(|p| p.into_inner()) = Some(client);
    Ok(())
}

/// Parse an optional interface-IP string ("" / null → None). Errors on a
/// non-empty value that isn't a valid IP.
fn parse_iface(iface: Option<String>) -> Result<Option<IpAddr>, String> {
    match iface.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(s) => s
            .parse::<IpAddr>()
            .map(Some)
            .map_err(|_| format!("'{s}' is not a valid interface IP address")),
    }
}

#[tauri::command]
fn get_state(state: tauri::State<Shared>) -> serde_json::Value {
    let s = state.lock().unwrap_or_else(|p| p.into_inner());
    serde_json::json!({ "topics": dashboard::snapshot(&s.snap, &s.unavailable) })
}

#[tauri::command]
fn get_meta(state: tauri::State<Shared>) -> serde_json::Value {
    let s = state.lock().unwrap_or_else(|p| p.into_inner());
    serde_json::json!({
        "backend": "ros2",
        "domain": s.domain,
        "iface": s.iface.map(|i| i.to_string()),
        "discovered": s.discovered,
        "connected": s.connected,
        "error": s.error,
        "watchdog_s": mingoros_core::dv_contract::STALENESS_WATCHDOG_S,
    })
}

/// The host's network interfaces — for the app's interface picker (pick the
/// direct-link Ethernet to bind DDS to instead of typing its IP).
#[tauri::command]
fn list_interfaces() -> Vec<mingoros_core::net::NetInterface> {
    mingoros_core::net::list_interfaces()
}

#[tauri::command]
fn connect(
    domain: u16,
    iface: Option<String>,
    state: tauri::State<Shared>,
    client: tauri::State<ClientCell>,
    echo: tauri::State<EchoCell>,
) -> Result<(), String> {
    let iface = parse_iface(iface)?;
    if let Err(e) = connect_impl(domain, iface, &state, &client, &echo) {
        state.lock().unwrap_or_else(|p| p.into_inner()).error = Some(e.clone());
        return Err(e);
    }
    Ok(())
}

/// Call the uDV's `/force_ebs` service (std_srvs/SetBool). `engage=true` fires
/// the Emergency Brake System (car-on-stands checkup); `false` returns it to
/// normal. The frontend gates this behind an explicit confirmation. Locks the
/// client cell only — never `AppState` — so the dashboard keeps updating.
#[tauri::command]
fn force_ebs(engage: bool, client: tauri::State<ClientCell>) -> Result<serde_json::Value, String> {
    let guard = client
        .lock()
        .map_err(|_| "client state lock poisoned — reconnect and retry".to_string())?;
    let c = guard
        .as_ref()
        .ok_or_else(|| "not connected to a ROS graph yet".to_string())?;
    let out = c
        .call_set_bool(mingoros_core::dv_contract::SERVICE_FORCE_EBS, engage)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "success": out.success, "message": out.message }))
}

// ---- Generic echo tab ------------------------------------------------------
//
// A running echo session pumps `subscribe_raw` (any topic — contract types
// richly, standard ROS types by name, unknown types as liveness) into a ring
// buffer on a background thread; the frontend polls `echo_tail`. Held in its
// own cell, entirely separate from the safety board, so echoing an arbitrary
// topic never disturbs the dashboard poll.

const ECHO_CAP: usize = 2000;

/// One subscribed topic in the echo view. Its background thread pumps
/// `subscribe_raw` samples into the SHARED ring buffer; several topics thus
/// interleave in one merged stream, each row tagged by `Sample.topic`.
struct EchoTopic {
    topic: String,
    stop: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

/// The echo session: the shared merged ring buffer + the set of active topics.
#[derive(Default)]
struct EchoState {
    buf: Arc<Mutex<VecDeque<Sample>>>,
    topics: Vec<EchoTopic>,
}

type EchoCell = Mutex<EchoState>;

/// Topics currently visible on the graph — for the echo tab's topic picker.
#[tauri::command]
fn list_topics(
    client: tauri::State<ClientCell>,
) -> Result<Vec<mingoros_core::ros::TopicInfo>, String> {
    let guard = client.lock().unwrap_or_else(|p| p.into_inner());
    let c = guard
        .as_ref()
        .ok_or_else(|| "not connected to a ROS graph yet".to_string())?;
    c.list_topics().map_err(|e| e.to_string())
}

/// Add `topic` to the echo view (idempotent). Spawns a thread pumping
/// `subscribe_raw` into the shared, topic-tagged ring buffer until it is
/// removed/cleared or the topic goes silent.
#[tauri::command]
fn echo_add(
    topic: String,
    client: tauri::State<ClientCell>,
    echo: tauri::State<EchoCell>,
) -> Result<(), String> {
    // Already echoing this topic? idempotent no-op (avoids a duplicate reader).
    if echo
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .topics
        .iter()
        .any(|t| t.topic == topic)
    {
        return Ok(());
    }
    // Subscribe under the client lock, then release it before pumping.
    let mut stream = {
        let guard = client.lock().unwrap_or_else(|p| p.into_inner());
        let c = guard
            .as_ref()
            .ok_or_else(|| "not connected to a ROS graph yet".to_string())?;
        c.subscribe_raw(&topic).map_err(|e| e.to_string())?
    };
    let stop = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));
    {
        let mut e = echo.lock().unwrap_or_else(|p| p.into_inner());
        // Re-check under the lock: a concurrent add of the same topic could
        // have won the race between our first check and here.
        if e.topics.iter().any(|t| t.topic == topic) {
            return Ok(()); // `stream` drops, tearing down the extra reader
        }
        let buf = e.buf.clone();
        let (bstop, brunning) = (stop.clone(), running.clone());
        std::thread::spawn(move || {
            while !bstop.load(Ordering::Relaxed) {
                match stream.next_sample() {
                    Some(s) => {
                        let mut b = buf.lock().unwrap_or_else(|p| p.into_inner());
                        if b.len() >= ECHO_CAP {
                            b.pop_front();
                        }
                        b.push_back(s);
                    }
                    // ~20s of silence or a reader error ends this topic's stream.
                    None => break,
                }
            }
            brunning.store(false, Ordering::Relaxed);
        });
        e.topics.push(EchoTopic {
            topic,
            stop,
            running,
        });
    }
    Ok(())
}

/// Remove one topic from the echo view (stops its thread; keeps the buffer).
#[tauri::command]
fn echo_remove(topic: String, echo: tauri::State<EchoCell>) {
    let mut e = echo.lock().unwrap_or_else(|p| p.into_inner());
    e.topics.retain(|t| {
        if t.topic == topic {
            t.stop.store(true, Ordering::Relaxed);
            false
        } else {
            true
        }
    });
}

/// Stop every echo topic and clear the merged buffer.
fn clear_echo(echo: &EchoCell) {
    let mut e = echo.lock().unwrap_or_else(|p| p.into_inner());
    for t in &e.topics {
        t.stop.store(true, Ordering::Relaxed);
    }
    e.topics.clear();
    e.buf.lock().unwrap_or_else(|p| p.into_inner()).clear();
}

/// Stop every topic and clear the merged buffer.
#[tauri::command]
fn echo_clear(echo: tauri::State<EchoCell>) {
    clear_echo(echo.inner());
}

/// The merged tail across all active topics (last `limit`, oldest→newest) plus
/// each topic's running flag.
#[tauri::command]
fn echo_tail(limit: usize, echo: tauri::State<EchoCell>) -> serde_json::Value {
    let e = echo.lock().unwrap_or_else(|p| p.into_inner());
    let b = e.buf.lock().unwrap_or_else(|p| p.into_inner());
    // Only surface samples for currently-active topics. A just-removed or
    // just-cleared topic can have one last sample pushed by its thread before
    // it observes the stop flag; filtering by the live topic set keeps those
    // stragglers (and any removed topic's history) out of the view.
    let active: HashSet<&str> = e.topics.iter().map(|t| t.topic.as_str()).collect();
    let filtered: Vec<&Sample> = b
        .iter()
        .filter(|s| active.contains(s.topic.as_str()))
        .collect();
    let start = filtered.len().saturating_sub(limit);
    let samples = &filtered[start..];
    let topics: Vec<serde_json::Value> = e
        .topics
        .iter()
        .map(|t| serde_json::json!({ "topic": t.topic, "running": t.running.load(Ordering::Relaxed) }))
        .collect();
    serde_json::json!({ "topics": topics, "total": b.len(), "samples": samples })
}

/// App entry — called from `main.rs`.
pub fn run() {
    let client_cell: ClientCell = Mutex::new(None);
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(Mutex::new(AppState::default()))
        .manage(client_cell)
        .manage(EchoCell::default())
        .setup(|app| {
            // Auto-connect to domain 0 (all interfaces) at launch; a failure is
            // surfaced in get_meta().error rather than blocking the window.
            let state = app.state::<Shared>();
            let client = app.state::<ClientCell>();
            let echo = app.state::<EchoCell>();
            if let Err(e) = connect_impl(0, None, state.inner(), client.inner(), echo.inner()) {
                state.lock().unwrap_or_else(|p| p.into_inner()).error = Some(e);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_meta,
            connect,
            force_ebs,
            list_interfaces,
            list_topics,
            echo_add,
            echo_remove,
            echo_clear,
            echo_tail
        ])
        .run(tauri::generate_context!())
        .expect("error while running ISC MingoROS");
}
