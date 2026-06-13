FROM rust:1.94-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release

FROM debian:bookworm-slim
RUN useradd --system --create-home --shell /usr/sbin/nologin floorplan \
  && mkdir -p /var/lib/glb-floorplan/artifacts \
  && chown -R floorplan:floorplan /var/lib/glb-floorplan
USER floorplan
WORKDIR /app
COPY --from=builder /app/target/release/glb-floorplan-backend /usr/local/bin/glb-floorplan-backend
EXPOSE 8080
CMD ["glb-floorplan-backend"]
