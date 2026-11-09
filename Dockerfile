# Multi-stage Dockerfile for SheafDB (ShelfDB)
FROM rust:latest as builder

WORKDIR /usr/src/sheaf-db

# Copy workspace crates and config
COPY sheaf-db/Cargo.toml sheaf-db/Cargo.lock ./
COPY sheaf-db/crates ./crates

# Build release application binary
RUN cargo build --bin sheaf_db_app --release

# Minimal runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /usr/src/sheaf-db/target/release/sheaf_db_app /usr/local/bin/sheaf_db_app

# Copy default configurations
COPY sheaf-db/tests/e2e/realtime_bench/tql_workloads.conf /etc/sheaf-db/tql_workloads.conf

# Set environment
ENV PATH="/usr/local/bin:${PATH}"

# Default entrypoint executes the database application
ENTRYPOINT ["sheaf_db_app"]
