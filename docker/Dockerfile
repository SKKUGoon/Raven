# Multi-stage Rust build for Project Raven
# Stage 1: Build dependencies and cache them
FROM rust:1.88-slim AS chef
WORKDIR /app
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-chef for dependency caching
RUN cargo install cargo-chef

# Stage 2: Prepare recipe for dependency caching
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build dependencies (cached layer)
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source code and build the application
COPY . .
RUN cargo build --release --bin market-data-server

# Stage 4: Runtime image
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user for security
RUN groupadd -r raven && useradd -r -g raven -s /bin/false raven

# Create necessary directories
RUN mkdir -p /app/config /app/logs /app/data \
    && chown -R raven:raven /app

WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/market-data-server /app/
COPY --from=builder /app/config/ /app/config/

# Copy health check script
COPY docker/healthcheck.sh /app/
RUN chmod +x /app/healthcheck.sh

# Set ownership
RUN chown -R raven:raven /app

# Switch to non-root user
USER raven

# Expose gRPC port
EXPOSE 50051

# Health check using custom script
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD /app/healthcheck.sh quick || exit 1

# Set environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
ENV CONFIG_PATH=/app/config

# Run the application
CMD ["/app/market-data-server"]