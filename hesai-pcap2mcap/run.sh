#!/usr/bin/env bash
# Convert a Hesai LiDAR .pcap capture into an MCAP bag that opens directly in
# Lichtblick / Foxglove (the IFSSIM repo ships lichtblick/hesai_pcap_replay.json
# as a starter layout for the resulting bag).
#
# Why this exists:
#   A raw Hesai pcap is just UDP packets — no MCAP reader understands the
#   Pandar/AT/ATX UDP protocols. We need to replay the pcap through the Hesai
#   driver to get sensor_msgs/PointCloud2 frames, and record those into MCAP.
#   This wrapper handles the docker-build-if-missing dance and the volume
#   mount so the operator can just point it at a file.
#
# Usage:
#   ./run.sh <path/to.pcap> [output_dir]
#
# Examples:
#   ./run.sh ~/Downloads/test1.pcap
#       -> bag at ~/Downloads/test1_mcap/test1_mcap_0.mcap
#
#   ./run.sh ~/Downloads/test1.pcap bags/hesai_test1
#       -> bag at bags/hesai_test1/<name>_0.mcap
#
# Env (override before invocation):
#   RECORD_SECONDS    wall-clock recording cap, default = pcap_duration + 15
#   CORR_FILE         angle-correction file path (inside container).
#                     Default: ATX (UDP protocol v4.7). For other Hesai models,
#                     `docker run --rm --entrypoint ls hesai-pcap2mcap:latest \
#                       /opt/hesai/correction/angle_correction/` lists choices.
#   FIRE_FILE         firetime-correction file path (inside container).
#   IMAGE             override the Docker image tag (default: hesai-pcap2mcap:latest)

set -euo pipefail

IMAGE="${IMAGE:-hesai-pcap2mcap:latest}"
HERE="$(cd "$(dirname "$0")" && pwd)"

if [[ $# -lt 1 ]]; then
  sed -n '2,/^set -euo/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//'
  exit 1
fi

PCAP_HOST="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
[[ -f "$PCAP_HOST" ]] || { echo "pcap not found: $1" >&2; exit 1; }

if [[ $# -ge 2 ]]; then
  OUT_HOST_DIR="$2"
  mkdir -p "$(dirname "$OUT_HOST_DIR")" 2>/dev/null || true
  OUT_HOST_DIR="$(cd "$(dirname "$OUT_HOST_DIR")" && pwd)/$(basename "$OUT_HOST_DIR")"
else
  PCAP_BASE="$(basename "$PCAP_HOST" .pcap)"
  OUT_HOST_DIR="$(dirname "$PCAP_HOST")/${PCAP_BASE}_mcap"
fi

# Build the image on first use (or whenever the user has nuked it).
if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  echo "[hesai-pcap2mcap] image $IMAGE not found; building..."
  docker build -t "$IMAGE" "$HERE"
fi

# Derive a default record duration from the pcap. tcpdump from macOS Downloads
# is TCC-blocked on the host but works inside a container where /data is a
# bind mount. We need this anyway to know when to stop the recorder.
if [[ -z "${RECORD_SECONDS:-}" ]]; then
  DURATION="$(docker run --rm \
    -v "$(dirname "$PCAP_HOST"):/data:ro" \
    --entrypoint sh "$IMAGE" -c "
      apt-get -qq install -y tcpdump >/dev/null 2>&1 || true
      first=\$(tcpdump -nn -r /data/$(basename "$PCAP_HOST") -tt -c 1 udp 2>/dev/null | awk '{print \$1}')
      last=\$(tcpdump -nn -r /data/$(basename "$PCAP_HOST") -tt udp 2>/dev/null | tail -1 | awk '{print \$1}')
      python3 -c \"print(int(float('\$last') - float('\$first')) + 15)\"
    " 2>/dev/null || true)"
  RECORD_SECONDS="${DURATION:-120}"
  echo "[hesai-pcap2mcap] auto-set RECORD_SECONDS=${RECORD_SECONDS}"
fi

DATA_HOST="$(dirname "$PCAP_HOST")"
PCAP_NAME="$(basename "$PCAP_HOST")"
OUT_NAME="$(basename "$OUT_HOST_DIR")"

# OUT_HOST_DIR must live in the same parent as PCAP for the single bind mount.
# Otherwise add a second -v and adjust the OUT_DIR path passed to the entrypoint.
if [[ "$(dirname "$OUT_HOST_DIR")" != "$DATA_HOST" ]]; then
  echo "[hesai-pcap2mcap] note: output dir not under $(dirname "$PCAP_HOST"); using a second mount"
  EXTRA_MOUNT=(-v "$(dirname "$OUT_HOST_DIR"):/out")
  OUT_IN="/out/$OUT_NAME"
else
  EXTRA_MOUNT=()
  OUT_IN="/data/$OUT_NAME"
fi

docker run --rm \
  -v "$DATA_HOST:/data" \
  "${EXTRA_MOUNT[@]}" \
  -e PCAP="/data/$PCAP_NAME" \
  -e OUT_DIR="$OUT_IN" \
  -e RECORD_SECONDS="$RECORD_SECONDS" \
  ${CORR_FILE:+-e CORR_FILE="$CORR_FILE"} \
  ${FIRE_FILE:+-e FIRE_FILE="$FIRE_FILE"} \
  "$IMAGE"

echo
echo "[hesai-pcap2mcap] done. Bag: $OUT_HOST_DIR/"
echo "Open in Lichtblick with the layout at:"
echo "  https://github.com/isc-fs/IFSSIM/blob/dev/lichtblick/hesai_pcap_replay.json"
