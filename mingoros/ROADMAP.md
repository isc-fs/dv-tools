# MingoROS roadmap

Phased delivery, MVP → full tool. Each phase is a `feat/*` branch → PR → `dev`.
Derived from the scoping study of MingoCAN, WarioCharger, uDV and PIPELINE.

The guiding decision: **hybrid greenfield.** New workspace in MingoCAN's shape
(its CLI/GUI/packaging structure and house style), with the **ROS layer written
fresh** — there is zero Rust↔ROS2 precedent in any of the four repos, and that
is the project's real risk and its differentiator. Scope is **ROS-topic
debugging only**; raw CAN stays MingoCAN's job.

---

### ✅ Phase 0/1 — Scaffold + contract + fake backend  (`feat/1`, this branch)
- [x] Cargo workspace (`mingoros-core` + `mingoros-cli`), Rust 1.95, clean
      `clippy -D warnings` + `fmt`.
- [x] `dv_contract`: faithful port of the pipeline's `interface_contract.py`
      (AS/DV byte enums, mission registry, AMI→mission_id map), `fs_msgs/Cone`
      colours, the topic + QoS catalogue — with parity tests pinning the bytes.
- [x] `RosClient` transport trait + in-process `fake` backend.
- [x] CLI: `topics`, `echo`, `hz`, `pub` (with the actuation safety gate);
      `bag`/`adapters`/`monitor` stubbed with roadmap pointers.

### Phase 2 — ROS transport via ros2-client/RustDDS  (`feat/2`) — the biggest bet
- [x] **QoS-validation spike — PASS** (see [SPIKE.md](SPIKE.md)). Proven against
      IFSSIM's live `rmw_fastrtps` graph: cross-vendor discovery (~50 topics),
      RELIABLE delivery + CDR decode, and **latched TRANSIENT_LOCAL** retained
      delivery to a late joiner (t+21 ms). Kill-criterion cleared — no fallback
      to rclrs needed.
- [x] `ros2` backend implementing `RosClient` over ros2-client/RustDDS, behind
      the `ros2` feature; node spinner on a background thread.
- [x] `dv_contract` extended with the IFSSIM/sim topic surface; `topics` /
      `echo` / `hz` work against a live pipeline (Float32, fs_msgs/Track,
      std_msgs/Bool decoded).
- [x] Dockerfile — runs MingoROS in the pipeline's DDS domain (Linux/container).
- [ ] Large reliable samples: decode `visualization_msgs/MarkerArray` (`/Conos`)
      and `sensor_msgs/PointCloud2` — the reliable-*large* fragmentation case.
- [ ] Broaden typed decode (`nav_msgs/Odometry`, `sensor_msgs/Imu`, the uDV
      stock `std_msgs` byte topics) so `echo` covers the whole graph.

### Phase 3 — uDV link + flash
- [ ] `micro_ros_agent` subprocess manager (spawn/own the serial agent, surface
      XRCE session state + the ~10 s gyro-cal startup).
- [ ] Robust uDV detect (USB iSerial/product string or XRCE probe — VID/PID
      `0483:5740` is generic ST CDC, not unique).
- [ ] uDV **SWD/DFU** flash (probe-rs or CubeProgrammer/OpenOCD/dfu-util),
      hard-separated in the UI from the AMS/ECU CAN-bootloader flow.

### Phase 4 — Control plane + bag + dashboard
- [ ] Services/actions: `ActivateMode` (mode bring-up), `StartBag`/`StopBag`.
- [ ] Mission/AS/DV-state dashboard with the 1.5 s staleness watchdog.
- [ ] Cone-map + pose + path visualizer (colour-coded by `fs_msgs/Cone`).

### Phase 5 — GUI + release
- [ ] Tauri 2 + Svelte 5 shell (`apps/mingoros-studio`), forked from can-studio;
      streaming-command pattern; `ts-rs`/`tauri-specta` for Rust↔TS types.
- [ ] Signed cross-platform bundles (with the Linux-only feature matrix made
      explicit: the micro-ROS agent / rclrs are Linux+Pi only).

---

## Known risks (from the scoping study)
- **Rust↔ROS2 maturity.** `ros2-client`/RustDDS and `rclrs` are pre-1.0 with
  known gaps on `TRANSIENT_LOCAL` durability and large reliable-sample
  fragmentation — the exact QoS this pipeline leans on. Hence the Phase-2 spike.
- **QoS mismatch is silent no-data.** The full profile table is scattered across
  pipeline node sources; pin it in `dv_contract` as it's confirmed.
- **`foxglove_bridge` is sim-only** (launched only in the UE5 sim, whitelisted,
  excludes the mission-state bytes) — not a car transport, so not the MVP path.
- **uDV is SWD/DFU-flashed** (links at `0x08000000`, no CAN bootloader) —
  different from the AMS/ECU CAN-BL flow.
