# ISC MingoROS — the `ros2`-enabled CLI as a Linux image, so it can join the DV
# pipeline's DDS domain (RTPS/DDS over UDP, interoperating with rmw_fastrtps).
# Run it with `--network host` on the same domain as the pipeline (e.g. next to
# IFSSIM's dv_pipeline_stack). This is also how ISC MingoROS ships into the pipeline.
FROM rust:1.95-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
        libpcap-dev libudev-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release --features ros2 -p mingoros-cli

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libpcap0.8 libudev1 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/mingoros /usr/local/bin/mingoros
# Match the pipeline's DDS domain by default (override with -e ROS_DOMAIN_ID=N).
ENV ROS_DOMAIN_ID=0
ENTRYPOINT ["mingoros"]
