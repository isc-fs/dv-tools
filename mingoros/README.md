# ISC MingoROS

**ROS 2 topic debugger for the IFS08 Driverless stack.** MingoCAN
([`isc-fs/can-flasher`](https://github.com/isc-fs/can-flasher)) is to CAN frames
what ISC MingoROS is to ROS topics — a native **Go / No-Go safety board** for
commissioning a stopped car, plus a scriptable CLI (`topics` / `echo` / `hz` /
`pub` / `state` / `force-ebs` / …). One Rust tool instead of the scattered
`ros2 topic …` / `rqt` / Foxglove dance, and **no ROS install needed** on the
laptop (pure-Rust RustDDS).

Targets the [uDV](https://github.com/isc-fs/IFS08-DV-uDV) micro-ROS gateway and
the [DV pipeline](https://github.com/isc-fs/IFS08-DV_PIPELINE) ROS 2 graph.

> ### ⚠️ Safety — car on stands, wheels off the ground, **always**
> ISC MingoROS can command **actuation** — Force EBS, and `pub` to control / mission
> topics (a throttle command **will move the car**). A stray or mistaken command
> must never be able to drive the wheels. Only ever use ISC MingoROS on a car that is
> **jacked up and freewheeling**, never with the wheels able to touch down.

## Install

Download the latest from
[**Releases**](https://github.com/isc-fs/dv-tools/releases) (tag `mingoros-v*`):

| | macOS | Linux | Windows |
|---|---|---|---|
| **ISC MingoROS** desktop app | `.dmg` (universal) | `.deb` / `.AppImage` / `.rpm` | `-setup.exe` (NSIS) |
| **`mingoROS`** CLI | binary (aarch64/x86_64) | binary (x86_64/aarch64) | `.exe` |

macOS `.dmg` is ad-hoc signed — first launch needs **right-click → Open →
confirm**. The Windows build needs **[Npcap](https://npcap.com)** installed (the
RustDDS transport uses pcap libraries there).

On **Linux** the desktop app needs **WebKitGTK 4.1** — **Ubuntu 22.04+** (20.04
is unsupported). Install the `.deb` with `sudo apt install ./ISC.MingoROS_*.deb`
(use `apt`, not `dpkg -i`, so the deps resolve); the `.AppImage` needs
`sudo apt install libfuse2`. The Linux build needs **no pcap**. Full per-format
steps + a troubleshooting table are in **[docs/INSTALL.md](docs/INSTALL.md)**.

To connect the laptop to the car's DV PC over a direct Ethernet cable, follow
**[docs/CONNECT.md](docs/CONNECT.md)** (static IPs · domain · interface bind).

## The mapping

| MingoCAN (CAN) | ISC MingoROS (ROS 2) |
|---|---|
| `adapters` — list CAN adapters | `topics` — list the graph: name, type, **QoS** |
| `monitor` — sniff bus frames | `echo` — live-subscribe + decode any topic |
| DBC decode | msg-type decode (`std_msgs` / `nav_msgs` / `fs_msgs` …) |
| `diagnose` / health | `hz` — rate/jitter; the `state` safety dashboard |
| `replay` — candump record/replay | `bag` — rosbag record/replay |
| `send-raw` — inject a frame | `pub` / `force-ebs` — inject / actuate (safety-gated) |

## The desktop app

**ISC MingoROS** (Tauri 2 + Svelte 5) is the graphical companion — a double-click
executable like MingoCAN's `can-studio`. It joins the car's DDS graph over
Ethernet and renders a live **Go / No-Go board**: a dominant AS-state readout, a
`READY TO DRIVE / NOT READY / FAULT` verdict that names the blocking interlocks,
the twelve `/debug` safety signals as **PASS / FAIL / HOLD** checklists, a RES
e-stop bar, and a discovered-topic count in the connection bar. A guarded
**Force EBS** control fires the emergency brake for a car-on-stands checkup
(behind a confirmation).

A second **Topic echo** tab echoes *any* topics on the graph — add one or several
from the discovered list or by path, and their messages interleave in one
colour-coded stream (per-topic rate + count). Standard ROS types decode to
readable fields; anything else still shows liveness (arrival + rate + type). The
safety strip stays pinned across both tabs.

```bash
cd mingoros/apps/mingoros-studio
npm install && npx @tauri-apps/cli icon src-tauri/icons/icon.png
npm run tauri:dev            # run it; npm run tauri:build to bundle
```

## The CLI

The `mingoROS` binary is the scriptable/headless companion; `--backend fake`
(default) needs no ROS, `--backend ros2` joins a live graph.

```bash
mingoROS topics                              # list the graph: name, type, QoS
mingoROS echo /dv/status --count 5           # decode the pipeline lifecycle bytes
mingoROS hz /assi/state --samples 20         # measure rate + jitter
mingoROS state                               # ★ live safety dashboard (terminal)
mingoROS pub /ami/mission 2 --force          # inject a message (actuation → --force)
mingoROS force-ebs on --force                # trigger the uDV /force_ebs service

# against a live car (see docs/CONNECT.md):
mingoROS topics --backend ros2 --domain 0 --iface 10.42.0.2
```

The **ros2 backend** (ros2-client / RustDDS, behind the `ros2` cargo feature)
interoperates with the pipeline's `rmw_fastrtps`; the QoS-validation spike
passed against IFSSIM (discovery, reliable, and latched TRANSIENT_LOCAL
delivery — see [SPIKE.md](SPIKE.md)). Typed decode covers the state bytes
(AS/DV/RES/mission), `nav_msgs/Odometry`, `sensor_msgs/Imu`,
`geometry_msgs/Twist`, `fs_msgs/ControlCommand`, and the uDV `/debug` string.
Perception/cones are out of scope.

## On the bench with a real uDV

The uDV is a micro-ROS (XRCE-DDS) endpoint on USB-CDC — it only appears on the
ROS graph once a `micro_ros_agent` bridges it. ISC MingoROS drives that:

```bash
mingoROS udv                       # detect the board (ranked USB candidates)
mingoROS agent --dev /dev/ttyACM0  # bridge it onto the graph (owns the agent)
mingoROS state --backend ros2      # live safety dashboard of the uDV
```

`udv` ranks ports on VID/PID `0483:5740` **plus** the USB product/serial name
(the generic-ST-CDC disambiguator); `agent` auto-detects when `--dev` is
omitted. Requires `micro_ros_agent` on `PATH`.

## Architecture

A Cargo workspace shaped like can-flasher — a core lib the CLI and the Tauri app
link by path:

```
mingoros/
├── crates/mingoros-core/          # the library
│   └── src/
│       ├── dv_contract.rs         # SINGLE SOURCE OF TRUTH — AS/DV state bytes,
│       │                          #   mission registry, AMI map, topic+QoS catalogue
│       ├── dashboard.rs           # shared safety-snapshot model (CLI + app)
│       ├── agent.rs               # uDV detect + micro_ros_agent argv
│       ├── msgs.rs                # (ros2) serde ROS 2 msg types for CDR decode
│       └── ros/                   # ROS transport abstraction
│           ├── mod.rs             #   RosClient trait + TopicInfo/Sample
│           ├── fake.rs            #   in-process synthetic graph
│           └── ros2.rs            #   (ros2) ros2-client/RustDDS live backend
├── crates/mingoros-cli/           # the `mingoROS` binary (clap)
├── apps/mingoros-studio/          # the ISC MingoROS desktop app (Tauri 2 + Svelte 5)
│   ├── src/                       #   Svelte frontend (the Go/No-Go board)
│   └── src-tauri/                 #   Rust: mingoros-core in a window (see its README)
└── docs/CONNECT.md                # laptop ↔ DV PC direct-cable setup
```

- **`RosClient`** is the seam every backend implements — `fake` and `ros2`
  (ros2-client / RustDDS) behind a cargo feature. Nothing above this layer knows
  which transport it's driving.
- **`dv_contract`** keeps ISC MingoROS in lockstep with the firmware (C) and the
  pipeline (Python). Its `#[cfg(test)]` parity tests fail the build if a byte
  value drifts.
- The **safety gate** (`pub` requires `--force` for command/actuation topics;
  `force-ebs on` requires `--force`) mirrors can-flasher's CAN danger-frame
  deny-list.

## Develop

```bash
cargo test                                  # incl. dv_contract parity tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Toolchain pinned to Rust 1.95 (`rust-toolchain.toml`), matching can-flasher. The
Tauri app is a workspace member but **not** a default member, so plain
`cargo build` / the core+CLI CI stay Tauri-free.
