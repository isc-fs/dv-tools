#!/usr/bin/env python3
# Synthesise a v3 ATX_S01 angle correction `.dat` from the design values
# tabulated in Appendix A.1.1 of the ATX_S01 user manual (doc PV2-en-... A07).
#
# Why this exists:
#   Hesai ships a UNIQUE per-unit angle correction file with every sensor —
#   factory-measured to ~0.01°. Without it, the driver can't map packet
#   channel index → real (azimuth, elevation), so the output point cloud
#   looks like sheared noise (which is what we saw).
#
#   The proper fix is to pull the per-unit .dat from the sensor
#   (PTC command 0x05 on TCP :9347 or via PandarView 2). When that isn't
#   reachable, this script builds a generic ATX_S01 file from the manual's
#   design values. It won't match a specific unit to better than ~0.1°, and
#   the per-azimuth Elevation_Adjust array is set to zero (we don't have
#   those numbers without the per-unit file), but the channel-to-elevation
#   mapping is correct in structure — that alone should turn the cloud from
#   "garbled stripes" into "recognisable ground plane + cones".
#
# File format (Appendix A.2 of the manual, 181 + 6×N bytes for N channels):
#   off  size           field
#   0-1  2              0xEE 0xFF                    (delimiter)
#   2-3  2              0x04 0x03                    (Major.Minor = format v3)
#   4-5  2              reserved
#   6    1              ChannelNum = 0x74 (= 116)
#   7-8  2 LE uint      Resolution = 256             (=> unit is 1/256°)
#   9    2×N int16 LE   Even Azimuth Offset
#   9+2N 2×N int16 LE   Odd Azimuth Offset
#   9+4N 2×N int16 LE   Elevation
#   9+6N 2×70 int16 LE  Elevation_Adjust  (per-2°-azimuth correction; we
#                       leave zeros — generic file, no per-unit data)
#   end  32 bytes       SHA-256 over the preceding bytes
#
# Usage: synth_atx_s01_correction.py [output_path]
#   defaults to ./correction/ATX_S01_design_values.dat next to this script.

import hashlib
import pathlib
import struct
import sys

# Appendix A.1.1 — design values per Channel No. (1..116). Tuples are
# (even_horiz_offset_deg, odd_horiz_offset_deg, vertical_deg). Pulled from
# the manual; channel index in this list is `channel_number - 1`.
CHANNELS = [
    (0.82,  0.82,   5.92),  # Channel 1
    (0.82,  0.82,   5.52),  # Channel 2
    (0.82,  0.82,   5.12),
    (0.82,  0.82,   4.72),
    (-0.35, -0.35,  4.52),
    (-0.70, -0.70,  4.32),
    (-1.04, -1.04,  4.12),
    (-1.39, -1.39,  3.91),
    (-2.95, -2.95,  3.71),
    (0.54,  0.54,   3.51),  # Channel 10
    (-2.95, -2.95,  3.31),
    (0.54,  0.54,   3.11),
    (-2.94, -2.94,  2.90),
    (0.54,  0.54,   2.71),
    (-2.94, -2.94,  2.50),
    (0.54,  0.54,   2.30),
    (2.94,  2.94,   2.10),
    (-0.54, -0.54,  1.90),
    (2.94,  2.94,   1.70),
    (-0.54, -0.54,  1.50),  # Channel 20
    (2.94,  2.94,   1.30),
    (-0.54, -0.54,  1.10),
    (2.94,  2.94,   0.90),
    (-0.54, -0.54,  0.70),
    (-2.94, -2.94,  0.50),
    (0.54,  0.54,   0.30),
    (-2.94, -2.94,  0.10),
    (0.54,  0.54,  -0.10),
    (-2.94, -2.94, -0.30),
    (0.54,  0.54,  -0.50),  # Channel 30
    (-2.94, -2.94, -0.70),
    (0.54,  0.54,  -0.90),
    (2.94,  2.94,  -1.10),
    (-0.54, -0.54, -1.30),
    (2.94,  2.94,  -1.50),
    (-0.54, -0.54, -1.70),
    (2.94,  2.94,  -1.90),
    (-0.54, -0.54, -2.11),
    (2.95,  2.95,  -2.31),
    (-0.54, -0.54, -2.51),  # Channel 40
    (-2.95, -2.95, -2.71),
    (0.54,  0.54,  -2.91),
    (-2.95, -2.95, -3.11),
    (0.54,  0.54,  -3.32),
    (-2.95, -2.95, -3.52),
    (0.54,  0.54,  -3.72),
    (-2.95, -2.95, -3.92),
    (0.54,  0.54,  -4.13),
    (-0.54, -0.54, -4.53),
    (-0.54, -0.54, -4.93),  # Channel 50
    (-0.54, -0.54, -5.33),
    (-0.54, -0.54, -5.73),
    (0.54,  0.54,  -6.12),
    (0.54,  0.54,  -6.52),
    (0.54,  0.54,  -6.92),
    (0.54,  0.54,  -7.32),
    (-0.54, -0.54, -7.70),
    (-0.54, -0.54, -8.10),
    (-0.54, -0.54, -8.51),
    (-0.54, -0.54, -8.90),  # Channel 60
    (0.54,  0.54,  -9.46),
    (0.53,  0.53, -10.27),
    (-0.54, -0.54, -11.23),
    (-0.53, -0.53, -12.41),
    (-2.95,  0.54,  3.01),  # Channel 65 — pixel-enhancement region starts;
    (-2.94,  0.54,  2.80),  # asymmetric even/odd offsets here.
    (-2.94,  0.54,  2.60),
    (-2.94,  2.94,  2.40),
    (-2.94,  2.94,  2.20),
    (-0.54,  2.94,  2.00),  # Channel 70
    (-0.54,  2.94,  1.80),
    (-0.54,  2.94,  1.60),
    (-0.54,  2.94,  1.40),
    (-0.54,  2.94,  1.20),
    (-0.54,  2.94,  1.00),
    (-2.94,  2.94,  0.80),
    (-2.94,  2.94,  0.60),
    (-2.94,  0.54,  0.40),
    (-2.94,  0.54,  0.20),
    (-2.94,  0.54,  0.00),  # Channel 80 — boresight (vertical = 0°)
    (-2.94,  0.54, -0.20),
    (-2.94,  0.54, -0.40),
    (-2.94,  0.54, -0.60),
    (-2.94,  2.94, -0.80),
    (-2.94,  2.94, -1.00),
    (-0.54,  2.94, -1.20),
    (-0.54,  2.94, -1.40),
    (-0.54,  2.94, -1.60),
    (-0.54,  2.94, -1.80),
    (-0.54,  2.95, -2.01),  # Channel 90
    (-0.54,  2.95, -2.21),
    (-2.95,  2.95, -2.41),
    (-2.95,  2.95, -2.61),
    (-2.95,  0.54, -2.81),
    (-2.95,  0.54, -3.01),
    (-2.95,  0.54, -3.21),
    (-2.95,  0.54, -3.42),
    (-2.95,  0.54, -3.62),
    (-2.95,  0.54, -3.82),
    (-2.95,  0.54, -4.02),  # Channel 100
    (-2.95,  0.54, -4.33),
    (-0.54,  0.54, -4.73),
    (-0.54, -0.54, -5.13),
    (-0.54,  0.54, -5.53),
    (-0.54,  0.54, -5.93),
    (-0.54,  0.54, -6.32),
    (0.54,   0.54, -6.72),
    (-0.54,  0.54, -7.12),
    (-0.54,  0.54, -7.51),
    (-0.54,  0.54, -7.90),  # Channel 110
    (-0.54, -0.54, -8.31),
    (-0.54,  0.54, -8.71),
    (-0.54,  0.54, -9.18),
    (-0.54,  0.54, -9.87),
    (-0.54,  0.54, -10.75),
    (-0.54,  0.53, -11.82),  # Channel 116
]

RESOLUTION = 256
N_ELEV_ADJUST = 70


def deg_to_units(d: float) -> int:
    return int(round(d * RESOLUTION))


def build_dat() -> bytes:
    assert len(CHANNELS) == 116, f"expected 116 channels, got {len(CHANNELS)}"
    n = len(CHANNELS)

    buf = bytearray()
    buf += b"\xee\xff"                       # delimiter
    buf += b"\x04\x03"                       # protocol version major.minor = v3
    buf += b"\x00\x00"                       # reserved (2 bytes)
    buf += bytes([n])                        # ChannelNum = 116
    buf += struct.pack("<H", RESOLUTION)     # angle_division

    # Three int16-LE arrays in order: even az, odd az, elevation.
    for evn, _odd, _el in CHANNELS:
        buf += struct.pack("<h", deg_to_units(evn))
    for _evn, odd, _el in CHANNELS:
        buf += struct.pack("<h", deg_to_units(odd))
    for _evn, _odd, el in CHANNELS:
        buf += struct.pack("<h", deg_to_units(el))

    # Elevation_Adjust — per-2°-azimuth vertical correction. Generic file
    # has no per-unit data here; zero means "no adjustment", which is
    # what the SDK uses if it never receives an update.
    buf += b"\x00\x00" * N_ELEV_ADJUST

    # SHA-256 of everything preceding (chicken-and-egg: the SHA itself
    # is not in its own input).
    buf += hashlib.sha256(bytes(buf)).digest()

    assert len(buf) == 181 + 6 * n, f"size {len(buf)} != expected {181 + 6*n}"
    return bytes(buf)


def main() -> int:
    if len(sys.argv) > 1:
        out = pathlib.Path(sys.argv[1])
    else:
        out = pathlib.Path(__file__).parent / "correction" / "ATX_S01_design_values.dat"
    out.parent.mkdir(parents=True, exist_ok=True)
    data = build_dat()
    out.write_bytes(data)
    print(f"wrote {out} ({len(data)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
