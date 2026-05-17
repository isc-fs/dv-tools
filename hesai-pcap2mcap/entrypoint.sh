#!/usr/bin/env bash
# Replay a Hesai pcap through HesaiLidar_ROS_2.0 and record the resulting
# /lidar_points to an MCAP bag.
#
# Env (all optional):
#   PCAP              path inside the container (default: /data/in.pcap)
#   OUT_DIR           output bag directory      (default: /data/out_mcap)
#   RECORD_SECONDS    wall-clock recording cap  (default: 100)
#   CORR_FILE         angle-correction .dat/.csv for the sensor model
#   FIRE_FILE         firetime-correction file matching the sensor model
#   LIDAR_PORT        UDP destination port      (default: 2368)
#   FRAME_ID          ROS frame_id on outgoing PointCloud2 (default: hesai_lidar)
#   ROS_DOMAIN_ID     keeps us off the host's DV pipeline DDS graph (default: 77)
#
# CORR_FILE / FIRE_FILE defaults target ATX (UDP protocol v4.7). For other
# Hesai models override both — the SDK ships matching files under
# /opt/hesai/correction/ in the image.

set -eo pipefail

# Isolate from any other ROS 2 nodes running on the host (e.g. IFSSIM pipeline)
# so we only record what THIS container's driver publishes. Without this the
# recorder picks up every IFSSIM topic via DDS multicast and the bag balloons
# with /imu, /gps, /motor_rpm, etc.
export ROS_DOMAIN_ID="${ROS_DOMAIN_ID:-77}"
export ROS_LOCALHOST_ONLY=1

# ROS setup scripts reference unbound vars; source them before enabling -u.
source /opt/ros/humble/setup.bash
source /ws/install/setup.bash
set -u

PCAP="${PCAP:-/data/in.pcap}"
OUT_DIR="${OUT_DIR:-/data/out_mcap}"
RECORD_SECONDS="${RECORD_SECONDS:-100}"
CORR_FILE="${CORR_FILE:-/opt/hesai/correction/angle_correction/ATX_Angle_Correction_File_V42.dat}"
FIRE_FILE="${FIRE_FILE:-/opt/hesai/correction/firetime_correction/ATX_Firetime Correction File.csv}"
LIDAR_PORT="${LIDAR_PORT:-2368}"
FRAME_ID="${FRAME_ID:-hesai_lidar}"

if [[ ! -f "$PCAP" ]]; then
  echo "[hesai-pcap2mcap] pcap not found at $PCAP" >&2
  exit 1
fi
[[ -f "$CORR_FILE" ]] || { echo "[hesai-pcap2mcap] correction file missing: $CORR_FILE" >&2; exit 1; }
[[ -f "$FIRE_FILE" ]] || { echo "[hesai-pcap2mcap] firetime file missing: $FIRE_FILE" >&2; exit 1; }

TEMPLATE="$(find /ws -path '*/hesai_ros_driver/config/config.yaml' | head -1)"
[[ -n "$TEMPLATE" ]] || { echo "[hesai-pcap2mcap] could not locate driver config.yaml" >&2; exit 2; }

CFG="/tmp/config.yaml"
cp "$TEMPLATE" "$CFG"

# Patch the driver's config in place. The Hesai driver uses YamlRead<>() with a
# default fallback per key, so absent keys silently revert to live-LiDAR
# behaviour; the safest move is to touch the keys we know about explicitly.
python3 - "$CFG" "$PCAP" "$CORR_FILE" "$FIRE_FILE" "$LIDAR_PORT" "$FRAME_ID" <<'PY'
import sys, yaml, pathlib
cfg_path, pcap, corr, fire, lport, fid = sys.argv[1:7]
cfg = yaml.safe_load(pathlib.Path(cfg_path).read_text())
s = cfg["lidar"][0]
drv = s["driver"]

drv["source_type"] = 2                       # 2 = pcap

pt = drv.setdefault("pcap_type", {})
pt["pcap_path"] = pcap
pt["correction_file_path"] = corr
pt["firetimes_path"] = fire
pt["pcap_play_synchronization"] = True
pt["pcap_play_in_loop"] = False
pt["play_rate_"] = 1.0

# udp_type still gets read in pcap mode; clear out anything that would make
# the driver try to talk to a live sensor.
ut = drv.setdefault("lidar_udp_type", {})
ut["udp_port"] = int(lport)
ut["use_ptc_connected"] = False
ut["multicast_ip_address"] = ""

ros = s["ros"]
ros["ros_frame_id"] = fid
ros["ros_send_point_cloud_topic"] = "/lidar_points"
ros["send_point_cloud_ros"] = True
ros["send_imu_ros"] = False
ros["send_packet_ros"] = False

pathlib.Path(cfg_path).write_text(yaml.safe_dump(cfg, sort_keys=False))
PY

mkdir -p "$(dirname "$OUT_DIR")"
rm -rf "$OUT_DIR"

# The shipped launch wrapper (start.py) also spawns rviz2, which we don't
# install in this headless image — it would crash the launch on import. Run
# the driver node directly and pass our patched config explicitly; the node
# otherwise reads from PROJECT_PATH/config/config.yaml (a path baked in at
# build time, NOT our /tmp/config.yaml).
ros2 run hesai_ros_driver hesai_ros_driver_node \
  --ros-args -p config_path:="$CFG" &
DRV_PID=$!
sleep 5

ros2 bag record -s mcap -o "$OUT_DIR" /lidar_points /lidar_imu /lidar_packets_loss &
REC_PID=$!

echo "[hesai-pcap2mcap] recording lidar topics for ${RECORD_SECONDS}s..."
sleep "$RECORD_SECONDS"

echo "[hesai-pcap2mcap] stopping recorder + driver"
kill -INT "$REC_PID" 2>/dev/null || true
wait  "$REC_PID" 2>/dev/null || true
kill -INT "$DRV_PID" 2>/dev/null || true
wait  "$DRV_PID" 2>/dev/null || true

echo "[hesai-pcap2mcap] mcap files produced:"
find "$OUT_DIR" -maxdepth 2 -name '*.mcap' -print
