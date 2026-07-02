# MingoROS Studio

The native desktop dashboard — MingoCAN's `can-studio` parallel for ROS. A
Tauri 2 app: the same `mingoros-core` engine as the CLI, in a window.

It runs on the **laptop** and joins the car PC's ROS 2 DDS graph directly over
**Ethernet** (the `ros2` / RustDDS backend). The window renders the shared
`ui/` frontend (a live safety/state dashboard); the frontend calls the Rust
commands (`get_state`, `get_meta`, `connect`) over Tauri IPC.

```
apps/mingoros-studio/
├── ui/                 # shared frontend (also served by `mingoros serve`)
│   └── index.html
└── src-tauri/          # the Tauri app (Rust)
    ├── src/main.rs     #   AppState + commands; auto-connects domain 0 at launch
    ├── tauri.conf.json #   window + withGlobalTauri (so the UI can invoke)
    └── icons/
```

## Run

Needs the Tauri CLI once:

```bash
cargo install tauri-cli --version '^2'      # or: npm i -g @tauri-apps/cli
cd mingoros/apps/mingoros-studio/src-tauri
cargo tauri dev                             # launches the window (debug)
cargo tauri build                           # bundles a .app / installer
```

Set the car's `ROS_DOMAIN_ID` in the connection bar (default 0). Wired
same-subnet Ethernet lets RustDDS discover the graph via multicast.

`cargo build -p mingoros-studio` compiles the Rust without launching the
window (what CI/quick checks use). The app is a workspace member but **not** a
default member, so plain `cargo build` / the core+CLI CI don't pull the Tauri
toolchain.

## Connection note

The desktop app does DDS **over the network** to the car — fine on wired,
same-subnet Ethernet. Over WiFi, prefer running `mingoros serve` on the car PC
and opening the dashboard in a browser (DDS-over-WiFi is unreliable). Same
frontend either way.
