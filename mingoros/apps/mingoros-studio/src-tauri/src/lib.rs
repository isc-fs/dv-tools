//! MingoROS Studio — Tauri command surface + app body.
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
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Shared app state: the live snapshot + the DDS client kept alive.
struct AppState {
    snap: Snapshot,
    unavailable: Vec<&'static str>,
    // Held so the DDS node stays alive for the RX threads' lifetime.
    _client: Option<Box<dyn RosClient>>,
    domain: u16,
    connected: bool,
    error: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            snap: Arc::new(Mutex::new(HashMap::new())),
            unavailable: Vec::new(),
            _client: None,
            domain: 0,
            connected: false,
            error: None,
        }
    }
}

type Shared = Mutex<AppState>;

/// Build the transport: the real ros2/RustDDS client, or the in-process fake
/// (set `MINGOROS_FAKE=1`) for demos / development without a ROS graph.
fn make_client(domain: u16) -> Result<Box<dyn RosClient>, String> {
    if std::env::var_os("MINGOROS_FAKE").is_some() {
        return Ok(Box::new(mingoros_core::ros::fake::FakeRos::new()));
    }
    std::env::set_var("ROS_DOMAIN_ID", domain.to_string());
    Ok(Box::new(
        mingoros_core::ros::ros2::Ros2Client::new().map_err(|e| e.to_string())?,
    ))
}

/// Join (or re-join) the ROS 2 graph on `domain` and start the subscribers.
fn connect_impl(domain: u16, state: &Shared) -> Result<(), String> {
    let client = make_client(domain)?;
    let snap: Snapshot = Arc::new(Mutex::new(HashMap::new()));
    let unavailable = dashboard::spawn_subscribers(client.as_ref(), &snap);
    let mut s = state.lock().unwrap();
    s.snap = snap;
    s.unavailable = unavailable;
    s._client = Some(client);
    s.domain = domain;
    s.connected = true;
    s.error = None;
    Ok(())
}

#[tauri::command]
fn get_state(state: tauri::State<Shared>) -> serde_json::Value {
    let s = state.lock().unwrap();
    serde_json::json!({ "topics": dashboard::snapshot(&s.snap, &s.unavailable) })
}

#[tauri::command]
fn get_meta(state: tauri::State<Shared>) -> serde_json::Value {
    let s = state.lock().unwrap();
    serde_json::json!({
        "backend": "ros2",
        "domain": s.domain,
        "connected": s.connected,
        "error": s.error,
        "watchdog_s": mingoros_core::dv_contract::STALENESS_WATCHDOG_S,
    })
}

#[tauri::command]
fn connect(domain: u16, state: tauri::State<Shared>) -> Result<(), String> {
    if let Err(e) = connect_impl(domain, &state) {
        state.lock().unwrap().error = Some(e.clone());
        return Err(e);
    }
    Ok(())
}

/// App entry — called from `main.rs`.
pub fn run() {
    tauri::Builder::default()
        .manage(Mutex::new(AppState::default()))
        .setup(|app| {
            // Auto-connect to domain 0 at launch; a failure is surfaced in
            // get_meta().error rather than blocking the window.
            let state = app.state::<Shared>();
            if let Err(e) = connect_impl(0, state.inner()) {
                state.lock().unwrap().error = Some(e);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_state, get_meta, connect])
        .run(tauri::generate_context!())
        .expect("error while running MingoROS Studio");
}
