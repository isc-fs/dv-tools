# Connecting MingoROS to the DV PC (direct RJ45 cable)

MingoROS joins the car's ROS 2 DDS graph directly — no ROS install on the laptop
(pure-Rust RustDDS). This is the setup for a **laptop ↔ DV PC point-to-point
Ethernet cable**, the field configuration.

## The one thing to know first

RustDDS (the DDS stack MingoROS uses) discovers peers over **UDP multicast**
(`239.255.0.1`) and has **no "connect to an IP" knob** — there is no unicast
discovery-server / static-peer option in this version. So you don't type the DV
PC's address. Instead you make the two machines **share a subnet** and make
**multicast reach across the cable**. MingoROS's lever is the **interface bind**:
point it at the laptop's direct-link NIC so discovery + data go over the cable
instead of WiFi.

## Recipe

Assume the direct cable is on `en7` (laptop, `ifconfig`/`ip addr` to find yours)
and `eth0` (DV PC), and you pick the subnet `10.42.0.0/24`.

### 1. Static IPs on both ends (a direct cable has no DHCP)

**DV PC (Linux):**
```bash
sudo ip addr add 10.42.0.1/24 dev eth0
sudo ip link set eth0 up
```

**Laptop (macOS):** System Settings → Network → the USB-Ethernet adapter →
Configure IPv4: **Manually**, IP `10.42.0.2`, mask `255.255.255.0` (no router).
(Linux laptop: `sudo ip addr add 10.42.0.2/24 dev en7 && sudo ip link set en7 up`.)

Verify the link: `ping 10.42.0.1` from the laptop.

### 2. Same ROS domain + let DDS off localhost (DV PC)

The pipeline uses **domain 0** with `rmw_fastrtps`. On the DV PC make sure DDS
isn't pinned to loopback:
```bash
export ROS_DOMAIN_ID=0
export ROS_LOCALHOST_ONLY=0        # important on multi-NIC machines
export RMW_IMPLEMENTATION=rmw_fastrtps_cpp
# then launch the pipeline as usual
```

### 3. Point MingoROS at the cable

- **Desktop app:** in the top bar set **domain = 0** and **iface = `10.42.0.2`**
  (the laptop's direct-link IP), then **Connect**. The backend label then shows
  `ros2 · dom 0 · 10.42.0.2 · N topics` — **N > 0 means the DV PC is reachable**.
- **CLI (preflight):**
  ```bash
  mingoros topics --backend ros2 --domain 0 --iface 10.42.0.2
  ```
  You should see the pipeline's topics (`/assi/state`, `/debug`, `/res/status`,
  …). `mingoros state --backend ros2 --domain 0 --iface 10.42.0.2` opens the live
  dashboard.

Binding to the interface makes RustDDS join + send discovery multicast on the
cable's NIC. If discovery still doesn't cross the link, add an explicit multicast
route so RTPS multicast egresses the cable:
```bash
# macOS (laptop)     — send the RTPS multicast group out the direct NIC
sudo route -nv add -net 239.0.0.0/8 -interface en7
# Linux (either end)
sudo ip route add 224.0.0.0/4 dev eth0
```

## Troubleshooting "no topics"

| Symptom | Likely cause | Fix |
|---|---|---|
| `ping` to the DV PC fails | IPs not on the same subnet / cable/NIC down | redo step 1; check `ifconfig`/`ip addr` |
| `0 topics`, ping OK | multicast not crossing the cable | set the **iface** bind (step 3); else add the multicast route |
| Some topics, not `/debug` | uDV agent not bridging | start `micro_ros_agent` on the DV PC (`mingoros agent`) |
| Topics on a laptop tool but not MingoROS | wrong **domain** | match `ROS_DOMAIN_ID` (pipeline = 0) |
| Connects then goes stale | firewall dropping DDS UDP | allow UDP 7400–7500 + ephemeral on both ends |

## Notes / limits

- **WiFi is not supported** for the data path — DDS multicast discovery + lossy
  reliable traffic over WiFi is unreliable. Keep it wired.
- **A switch instead of a direct cable** works too, as long as it forwards
  multicast (IGMP snooping off, or a querier present) and both ends share a
  subnet.
- **macOS + Docker Desktop:** a native Mac MingoROS **cannot** see a pipeline
  running inside Docker Desktop containers (the hypervisor doesn't pass DDS
  multicast). Test the `ros2` backend against a **real** DV PC or a Linux host,
  not IFSSIM-in-Docker on a Mac.
