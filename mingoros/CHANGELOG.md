# Changelog

## mingoros-v0.4.2

Single-viewport Go / No-Go board — operator feedback: the board should fill the
screen with live car state and not scroll. The old layout was 1380px tall at
1280×800 (a vertical scroll, all the horizontal space wasted). Reworked into two
tiers that use the width and lock the height to the viewport.

- **Verdict band** — a full-width header strip folds the overall-state word, the
  live-topic count, and the uDV-link badge into one slim line above the hero
  (replacing the old status banner). The AS-state readout + Go/No-Go stamp stay
  dominant.
- **Gauge deck** — the two 12-signal checklists (safety chain, drive readiness)
  sit side by side beside a compact gauge stack (RES bar, key-fact cards,
  pipeline roster, record toggle). The deck's height is its tallest column, not
  a vertical sum, which structurally removes the overflow.
- **New Details tab** — the raw-topic table and the session debrief move here,
  keeping the board itself glanceable. The recorder keeps capturing on every tab.

Result: fits 1280×800 / 1366×768 / 1440×900 with zero page scroll — all 12
signals + verdict + RES visible. Responsive: narrow screens collapse to one
column, short screens scroll the checklists/gauges internally as a safety valve.

## mingoros-v0.4.1

Connection honesty — the board now separates "DDS reachable" from "the car is
actually delivering data", so the status never claims CONNECTED when nothing is
coming from the car.

- **Green "connected" only on real data flow** — the LED goes green when a fresh
  priority-topic sample is actually arriving, not merely when the DDS participant
  comes up. A discovered topic can be the app's own subscription with no
  publisher, so the old "connected" lied whenever DDS was up but the car silent.
- **New amber "no data" state** — a distinct slow-pulse LED for DDS-reachable
  but no live data, clearly apart from green "connected" and red "offline".
- **Backend label shows `live/total` priority topics** instead of the misleading
  discovered-topic count (which also counted the app's own node and its
  subscriptions).
- **Pipeline roster gated on live data** — no stage reads "present" until data
  actually flows; the headline says "DDS is up, but no data is arriving" instead
  of implying a silent pipeline is there.

## mingoros-v0.4.0

The backlog release — the full brainstorm set, grounded in the real-bench
session. Highlights:

### Board / safety
- **Pipeline stage-up roster** — classifies live topics into DV stages
  (uDV/agent · Perception · SLAM · Planning · Control) and calls out
  "pipeline NOT launched — only the uDV agent is bridging".
- **Bound-NIC vanish alarm** + **link-loss folds into the verdict** — a
  silently-dropped interface goes loud (banner + LED) and forces the whole
  Go/No-Go verdict to FAULT.
- **uDV LINK health badge** (live / stale / down) from the heartbeat freshness.
- **RES-holder fullscreen** — a glanceable giant SAFE/HOLD/STOP for kill calls.
- **Stands interlock** — actuation (EBS + the new steering test) is locked
  until the operator confirms the car is on stands.
- **Steering self-test** — the `/activate_steering` counterpart to Force-EBS.
- **Session recorder + debrief** — records decoded-state transitions → a
  "what just happened" card.

### Topic echo
- **Numeric sparklines**, **per-topic rate-health** colouring, **header srcΔ**
  (source→arrival delay) on Odometry/Imu/PoseStamped, and a **QoS-mismatch
  explainer** for discovered-but-silent topics.

### CLI
- `mingoros codegen` (contract → Python/JSON), `doctor` (live contract linter +
  CI gate), `commission <spec.json>` (declarative pass/fail interlock runner),
  `bag list` + `bag play --loop/--rate` (bag library + cone-track E2E replay),
  and `export` (decoded samples → columnar CSV for pandas/DuckDB → Parquet).

### Diagnostics
- **Force-EBS failure taxonomy** — a timeout resolves to "not advertised"
  (uDV/agent down) vs "advertised but silent" (mapping/link) instead of an
  opaque hang.

## mingoros-v0.3.0

- **Echo multiple topics at once** in the Topic echo tab — add several topics and
  watch their messages interleave in one colour-coded stream, each tagged by
  topic with its own live count + rate. Remove topics individually (× on the
  chip) or clear all. Reconnecting resets the echo session.

## mingoros-v0.2.0

- **Generic "Topic echo" tab** in the desktop app — echo **any** topic on the
  graph, not just the DV-contract set. Pick from the discovered-topic list or
  type a path; standard ROS types (std_msgs scalars/String, common
  geometry_msgs / nav_msgs / sensor_msgs) decode to readable fields, and any
  other type still shows liveness (arrival + rate + type name). Backed by a new
  `subscribe_raw` transport path; the `mingoros echo` / `hz` CLI commands use it
  too, so they now work on arbitrary topics instead of only the contract set.
  (The board's Go/No-Go view is unchanged; a tab bar switches between them and
  the safety strip stays pinned on both.)

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
