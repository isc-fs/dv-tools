# Changelog

## mingoros-v0.1.0 — first release

The first release of **MingoROS** — the ROS 2 topic debugger for the IFS08
Driverless stack (MingoCAN, but for ROS topics). No ROS install needed on the
laptop (pure-Rust RustDDS).

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
