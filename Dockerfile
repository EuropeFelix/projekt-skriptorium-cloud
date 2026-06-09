# ============================
# Stage 1: Builder
# ============================
FROM rust:latest AS builder

WORKDIR /app

# Copy manifests first to cache dependency layers
COPY Cargo.toml Cargo.lock* ./

# Create a dummy main.rs so `cargo build` can fetch and compile dependencies
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true

# Now copy the actual source code
COPY src/ ./src/

# Fix the timestamp to force a rebuild of the actual source
RUN touch src/main.rs

# Build the release binary
RUN cargo build --release

# ============================
# Stage 2: Runner
# ============================
FROM debian:bullseye-slim AS runner

# Install runtime dependencies (for SQLite and basic system libs)
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libsqlite3-0 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Create a directory for the database file (can be mounted as a volume later)
RUN mkdir -p /app/data

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/scriptorium-cloud /app/scriptorium-cloud

# Copy static frontend files
COPY frontend/ ./frontend/

# Set the default database path to the data directory
ENV DATABASE_PATH=/app/data/scriptorium.db

# Expose the API port
EXPOSE 3000

# Run the binary
CMD ["./scriptorium-cloud"]