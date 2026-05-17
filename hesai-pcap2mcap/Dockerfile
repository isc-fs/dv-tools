# Hesai LiDAR pcap → MCAP converter.
#
# Why this exists:
#   * A Hesai pcap (raw Pandar/AT/ATX UDP capture) is not directly
#     openable in Lichtblick or Foxglove — it's UDP packets, not a
#     bag. We need to decode the Hesai protocol into PointCloud2 and
#     write it to MCAP before the rest of the inspection pipeline
#     can do anything with it.
#   * No off-the-shelf pip package decodes the full Hesai protocol
#     family (AT128, ATX, PandarXT, …). The official driver
#     HesaiLidar_ROS_2.0 handles all of them via a single binary that
#     dispatches on the protocol-header version, so we build it once
#     here and replay the pcap through it.
#   * The image is headless on purpose. The shipped launch file also
#     starts rviz2, which we can't run inside the container and don't
#     need anyway — we record straight to MCAP via rosbag2.
#
# Build:   docker build -t hesai-pcap2mcap:latest docker/hesai_pcap2mcap
# Usage:   see tools/hesai-pcap2mcap.sh and the entrypoint header.

FROM ros:humble-ros-base

ARG DEBIAN_FRONTEND=noninteractive
ENV LANG=C.UTF-8 LC_ALL=C.UTF-8

RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential cmake git pkg-config \
      python3-colcon-common-extensions python3-rosdep python3-vcstool \
      python3-yaml \
      libpcl-dev libyaml-cpp-dev libpcap-dev libboost-all-dev \
      ros-humble-pcl-conversions ros-humble-pcl-msgs ros-humble-pcl-ros \
      ros-humble-rosbag2 ros-humble-rosbag2-storage-mcap \
      ros-humble-sensor-msgs ros-humble-std-msgs \
    && rm -rf /var/lib/apt/lists/*

# Pin the driver via this build arg so a Hesai release that breaks the
# config schema doesn't silently land on `docker build`. Default tracks the
# upstream default branch (currently `master`) — override to a tag/SHA when
# you need reproducibility.
ARG HESAI_REF=master

WORKDIR /ws/src
RUN git clone --depth 1 --recurse-submodules \
      --branch "${HESAI_REF}" \
      https://github.com/HesaiTechnology/HesaiLidar_ROS_2.0.git hesai_ros_driver

# colcon would otherwise leave correction/firetime files only as symlinks into
# the source tree under build/; stash a stable copy before building so the
# entrypoint has a predictable path even if the build dir is pruned.
RUN mkdir -p /opt/hesai && \
    cp -r /ws/src/hesai_ros_driver/src/driver/HesaiLidar_SDK_2.0/correction /opt/hesai/

WORKDIR /ws
# Plain (non-symlink) install: the driver bakes PROJECT_PATH at compile time
# and looks for config.yaml under the source tree, so we must keep /ws/src
# populated. --symlink-install would also leave the executable as a dangling
# symlink the moment we prune build/, which is why this is a real install.
RUN bash -lc 'source /opt/ros/humble/setup.bash \
      && colcon build --cmake-args -DCMAKE_BUILD_TYPE=Release \
      && rm -rf build log'

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
