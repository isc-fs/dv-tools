# Changelog

## mingoros-v0.1.1

First follow-up to the initial release — no breaking changes.

- **Renamed to "ISC MingoROS"** across the app, CLI, docs, and window title
  (technical crate/dir slugs stay lowercase `mingoros-*`).
- **Network-interface dropdown** in the connection bar: pick the local interface
  for the direct-cable DV PC link from a live list instead of typing its IP
  (`mingoros ifaces` on the CLI). Direct-cable recipe in `docs/CONNECT.md`.
- **AS-state readout mirrors the car's ASSI light** — AS_READY solid yellow,
  AS_DRIVING flashing yellow, AS_FINISHED solid blue, AS_EMERGENCY flashing
  blue, AS_OFF grey — so the board reads the same as the light on the car.
- **Auto-update on launch** (`tauri-plugin-updater`): the app checks the newest
  **dv-tools GitHub Release** for a newer signed build and offers *Install &
  restart* (download → verify minisign signature → relaunch). Release CI signs
  the updater artifacts and attaches a combined `latest.json` (all platforms) to
  the same release — no cross-repo token needed. Setup in `docs/UPDATES.md`.
- **Windows now ships a single NSIS `-setup.exe`** (dropped the `.msi`) so the
  installed format matches what the auto-updater serves.

## mingoros-v0.1.0 — first release

The first release of **ISC MingoROS** — the ROS 2 topic debugger for the IFS08
Driverless stack (MingoCAN, but for ROS topics). No ROS install needed on the
laptop (pure-Rust RustDDS).

> ⚠️ **Safety:** ISC MingoROS can command actuation (EBS, control/mission topics).
> The car must be **on stands, wheels off the ground, at all times** while it is
> in use — a persistent banner in the app and notices in the docs enforce this.

### Desktop app (Tauri 2 + Svelte 5)
- **Go / No-Go safety board**: a dominant AS-state readout, a
  `READY TO DRIVE / NOT READY / FAULT` verdict that names the blocking
  interlocks, the twelve `/debug` safety signals as PASS/FAIL/HOLD checklists, a
  RES e-stop bar, key-fact cards, and a collapsible raw-topic table.
- **Connection bar**: ROS domain + local-interface bind (for a direct-cable DV
  PC link) + a discovered-topic count.
- **Force EBS** — a guarded actuation that fires the emergency brake for a
  car-on-stands checkup, behind a confirmation.
- Native bundles: macOS `.dmg` (universal), Linux `.deb`/`.AppImage`/`.rpm`,
  Windows `.msi`/`-setup.exe` (needs Npcap).

### CLI (`mingoros`)
- `topics` / `echo` / `hz` / `state` (terminal safety dashboard) / `pub`
  (safety-gated) / `force-ebs` / `udv` / `agent` / `bag`.
- `--backend fake` (no ROS) or `--backend ros2` (RustDDS); `--domain` / `--iface`
  for a direct-cable DV PC link.

### Core
- `dv_contract` — single source of truth for the AS/DV/RES state bytes, the
  mission registry, and the topic + QoS catalogue; parity-tested against the
  firmware (C) and pipeline (Python).
- `ros2` backend (ros2-client / RustDDS) — typed pub/sub + a `std_srvs/SetBool`
  service client (`/force_ebs`); interoperates with the pipeline's
  `rmw_fastrtps`.

See **[docs/CONNECT.md](docs/CONNECT.md)** to connect the laptop to the car over
a direct Ethernet cable.
