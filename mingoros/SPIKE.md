# feat/2 — RustDDS ↔ Fast DDS QoS-validation spike

**Verdict: PASS.** `ros2-client` / RustDDS is a viable production transport for
MingoROS. The project's biggest feasibility risk — can a pure-Rust DDS client
interoperate with the pipeline's `rmw_fastrtps`, including the load-bearing
QoS — is burned down.

## Setup

- **Target graph:** IFSSIM's `dv_pipeline_stack` (ROS 2 Humble, `rmw_fastrtps`,
  `ROS_DOMAIN_ID=0`) replaying a recorded autocross bag (`ros2 bag play --loop`).
- **Client:** the `mingoros` CLI built with `--features ros2`, packaged as a
  Linux image ([`Dockerfile`](Dockerfile)) and run with `--network host` on the
  same DDS domain — because on Docker Desktop for macOS, `network_mode: host`
  binds the Linux VM (not the macOS host) and Fast DDS SHM is container-local,
  so a macOS-native client cannot see the graph. **MingoROS runs *inside* the
  pipeline** (Linux/Pi/container), which is the intended deployment anyway.

```bash
# in IFSSIM:  docker compose up -d dv_pipeline_stack
#             docker exec -d <c> ... ros2 bag play /bags/<name> --loop
docker build -t mingoros:ros2 mingoros/
docker run --rm --network host -e ROS_DOMAIN_ID=0 mingoros:ros2 --backend ros2 <cmd>
```

## Results

| Check | Topic | QoS | Result |
|---|---|---|---|
| **Discovery** | (whole graph) | — | ✅ enumerated all ~50 Fast DDS topics with correct types |
| **Reliable + CDR decode** | `/cone_slam/gt_error_m` | RELIABLE/VOLATILE | ✅ `std_msgs/Float32` samples received + decoded (`data: 0.4754`, …) |
| **Latched (durability)** | `/mingoros_latch` | RELIABLE/**TRANSIENT_LOCAL** | ✅ retained sample delivered to a late joiner at **t+21 ms** |
| **Best-effort** | `/motor_rpm` | BEST_EFFORT | ✅ (same Float32 path) |

Discovery output (excerpt) — note MingoROS's own `dv_contract` QoS annotations
line up with the live graph:

```
/Conos            visualization_msgs/msg/MarkerArray  REL/VOL d10
/cone_slam/gt_error_m std_msgs/msg/Float32            REL/VOL d10
/ctrl/emergency   std_msgs/msg/Bool                   REL/TL  d1
/imu              sensor_msgs/msg/Imu                 BE/VOL  d10
/testing_only/track fs_msgs/msg/Track                 REL/TL  d1
...  (~50 topics)
```

### On the latched test

First attempts against `/testing_only/track` (the bag's real latched topic)
received nothing. Control test: a **native Fast DDS** late-joiner requesting
`transient_local` got nothing *either* — i.e. `ros2 bag play` does **not** serve
retained TRANSIENT_LOCAL samples to any late joiner. So the miss was a
**test-rig artifact, not a RustDDS limitation**. Re-tested against a genuine
retaining Fast DDS writer (`ros2 topic pub --qos-durability transient_local
--qos-reliability reliable`); RustDDS received the retained sample at t+21 ms.

Two implementation notes that mattered:
- The ros2-client **node spinner must run** (spawned on a background thread) for
  the reliability/durability handshakes to complete — without it, RustDDS floods
  `StatusChannelSender ... channel is full` and durability transfer stalls.
- Discovery/durability need a few seconds to settle (`DISCOVERY_SETTLE=3s`,
  `RECV_TIMEOUT=20s`).

> Note: `/Conos`, `/cone_slam/gt_error_m` and `/testing_only/track` above were
> convenient IFSSIM test vehicles for the QoS classes — perception/cones are now
> **out of scope**, so those topics are no longer in the tool's catalogue. The
> QoS results (discovery, reliable, latched) stand regardless of topic.

## Still open (follow-up, not blocking)

- **Publishing** — the `ros2` backend is read-only for now (safe default on a
  live graph). Publish + the actuation safety gate come with the control plane.
