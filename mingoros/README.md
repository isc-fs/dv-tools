# MingoROS

**ROS2 topic debugger for the IFS08 Driverless stack.** MingoCAN
([`isc-fs/can-flasher`](https://github.com/isc-fs/can-flasher)) is to CAN
frames what MingoROS is to ROS topics: `list` / `echo` / `hz` / `pub` /
`record`, plus (later) a live cone-map & mission-state dashboard — one Rust
tool instead of the scattered `ros2 topic …` / `rqt` / Foxglove dance.

Targets the [uDV](https://github.com/isc-fs/IFS08-DV-uDV) micro-ROS gateway and
the [DV pipeline](https://github.com/isc-fs/IFS08-DV_PIPELINE) ROS2 graph.

## The mapping

| MingoCAN (CAN) | MingoROS (ROS2) |
|---|---|
| `adapters` — list CAN adapters | `topics` — list the graph: name, type, **QoS** |
| `monitor` — sniff bus frames | `echo` — live-subscribe + decode any topic |
| DBC decode | msg-type decode (`dv_msgs`/`fs_msgs`/`std_msgs`) |
| `diagnose` / health | `hz` — rate/jitter; mission-state dashboard (later) |
| `replay` — candump record/replay | `bag` — rosbag record/replay (later) |
| `send-raw` — inject a frame | `pub` — inject a message (through a safety gate) |

## Status — `feat/1` scaffold

Working today (in-process **fake** backend, no ROS install needed):

```bash
cd mingoros
cargo run -- topics                      # list the known DV topics + QoS
cargo run -- echo /dv/status --count 5   # decode the pipeline lifecycle bytes
cargo run -- echo /ami/mission --count 5 # watch the AMI→mission_id mapping live
cargo run -- hz /assi/state --samples 20 # measure rate + jitter
cargo run -- pub /force_ebs true         # refused: actuation topic needs --force
```

The real DDS transport (`--backend ros2`) is **not built yet** — it lands in
`feat/2` after the QoS-validation spike (see [ROADMAP.md](ROADMAP.md)).

## Architecture

A Cargo workspace shaped like can-flasher — a core lib the CLI (and a future
Tauri GUI) link by path:

```
mingoros/
├── crates/mingoros-core/          # the library
│   └── src/
│       ├── dv_contract.rs         # SINGLE SOURCE OF TRUTH — AS/DV state bytes,
│       │                          #   mission registry, AMI map, topic + QoS
│       │                          #   catalogue. Ported from the pipeline's
│       │                          #   interface_contract.py + fs_msgs, with
│       │                          #   parity tests pinning every byte.
│       └── ros/                   # ROS transport abstraction
│           ├── mod.rs             #   RosClient trait + TopicInfo/Sample
│           └── fake.rs            #   in-process synthetic graph (this backend)
└── crates/mingoros-cli/           # the `mingoros` binary (clap)
```

- **`RosClient`** is the seam every backend implements — `fake` today, `ros2`
  (ros2-client / RustDDS) behind a cargo feature next. Nothing above this layer
  knows which transport it's driving.
- **`dv_contract`** keeps MingoROS in lockstep with the firmware (C) and the
  pipeline (Python). Its `#[cfg(test)]` parity tests fail the build if a byte
  value drifts.
- The **safety gate** (`pub` requires `--force` for command/actuation topics
  like `/force_ebs`, `/ami/mission`, `/ctrl/cmd`) mirrors can-flasher's CAN
  danger-frame deny-list.

## Develop

```bash
cargo test                              # incl. dv_contract parity tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Toolchain pinned to Rust 1.95 (`rust-toolchain.toml`), matching can-flasher.
