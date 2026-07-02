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
      (AS/DV byte enums, mission registry, AMI→mission_id map), the topic + QoS
      catalogue — with parity tests pinning the bytes.
- [x] `RosClient` transport trait + in-process `fake` backend.
- [x] CLI: `topics`, `echo`, `hz`, `pub` (with the actuation safety gate);
      `bag`/`adapters`/`monitor` stubbed with roadmap pointers.

> **Scope reality (car STOPPED):** MingoROS is used to commission a *stationary*
> car, so the priority surface is the **state machine + safety/mission signals**
> (AS state, ASMS, TS, SDC/RES, EBS, R2D, mission, `/dv/status`). Motion topics
> (pose, odom) are decoded for completeness; **perception/cones and LiDAR are
> out of scope entirely** (use rviz/Foxglove for those).

### Phase 2 — ROS transport via ros2-client/RustDDS  (`feat/2`) — the biggest bet
- [x] **QoS-validation spike — PASS** (see [SPIKE.md](SPIKE.md)). Proven against
      IFSSIM's live `rmw_fastrtps` graph: cross-vendor discovery (~50 topics),
      RELIABLE delivery + CDR decode, and **latched TRANSIENT_LOCAL** retained
      delivery to a late joiner (t+21 ms). Kill-criterion cleared — no fallback
      to rclrs needed.
- [x] `ros2` backend implementing `RosClient` over ros2-client/RustDDS, behind
      the `ros2` feature; node spinner on a background thread.
- [x] `dv_contract` extended with the IFSSIM/sim topic surface; `topics` /
      `echo` / `hz` work against a live pipeline (Float32 + std_msgs scalars
      decoded).
- [x] Dockerfile — runs MingoROS in the pipeline's DDS domain (Linux/container).
- [x] Corrected `dv_contract` against uDV `feat/15` + pipeline `feat/7` (right
      names/QoS; flagged the `/assi/state`+`/ami/mission` best-effort-vs-latched
      mismatch — filed IFS08-DV-PIPELINE#15). uDV state bytes decode to labels.
- [x] Broadened typed decode — live-verified vs IFSSIM: `nav_msgs/Odometry`
      (`/odom`, `/slam/pose` → x,y,yaw), `sensor_msgs/Imu` (`/imu`), plus
      `geometry_msgs/Twist` + `fs_msgs/ControlCommand` decoders. Prefix-struct
      trick reads leading fields, skipping `[f64;36]` covariance.
- [x] **Safety / state-machine surface (the priority for stopped-car bring-up):**
      `/debug` (the uDV dashboard string: AS ‖ ASMS/TS/SDC/EBS/ABS ‖
      brakes/mission/R2D/motion/finished ‖ RES ‖ EBSinit), `/res/status`
      (OK/ESTOP/GO/TIMEOUT/NONE), `/res/go`. `dv_contract` now mirrors the
      firmware's `AS_SIG_*` signal word (`as_state.h`) + RES codes, with a
      `describe_state_signals()` renderer and parity tests.

### Phase 3 — uDV link
> Firmware flashing is **out of scope** — MingoROS is a debugger/bridge, not a
> flasher. Flash the uDV with STM32CubeProgrammer / dfu-util as before.

- [x] Robust uDV detect (`mingoros udv`): enumerate USB serial ports, rank on
      VID/PID `0483:5740` + product/serial/manufacturer name hints (the
      generic-ST-CDC disambiguator). Pure ranking fn, unit-tested.
- [x] `micro_ros_agent` manager (`mingoros agent [--dev …]`): auto-detects the
      uDV, builds the argv, spawns/owns the bridge, surfaces the ~10 s gyro-cal
      startup + a clear error if the agent binary is absent. → full bench flow:
      `udv` → `agent --dev /dev/ttyACMx` → `state --backend ros2`.
- [ ] *(needs hardware to verify live)* XRCE-session state parsing from the
      agent's output; confirm the exact uDV product/iSerial string (usbd_desc.c).

### Phase 4 — Control plane + bag
- [ ] Services/actions: `ActivateMode` (mode bring-up), `StartBag`/`StopBag`.
- [ ] `bag` record/replay of a bench session (rosbag2 / mcap).

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
