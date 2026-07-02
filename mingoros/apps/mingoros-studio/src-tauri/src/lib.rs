//! MingoROS — Tauri command surface + app body.
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
use mingoros_core::ros::RosClient;
use std::collections::HashMap;
use std::net::IpAddr;
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
) -> Result<(), String> {
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

#[tauri::command]
fn connect(
    domain: u16,
    iface: Option<String>,
    state: tauri::State<Shared>,
    client: tauri::State<ClientCell>,
) -> Result<(), String> {
    let iface = parse_iface(iface)?;
    if let Err(e) = connect_impl(domain, iface, &state, &client) {
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

/// App entry — called from `main.rs`.
pub fn run() {
    let client_cell: ClientCell = Mutex::new(None);
    tauri::Builder::default()
        .manage(Mutex::new(AppState::default()))
        .manage(client_cell)
        .setup(|app| {
            // Auto-connect to domain 0 (all interfaces) at launch; a failure is
            // surfaced in get_meta().error rather than blocking the window.
            let state = app.state::<Shared>();
            let client = app.state::<ClientCell>();
            if let Err(e) = connect_impl(0, None, state.inner(), client.inner()) {
                state.lock().unwrap_or_else(|p| p.into_inner()).error = Some(e);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state, get_meta, connect, force_ebs
        ])
        .run(tauri::generate_context!())
        .expect("error while running MingoROS");
}
