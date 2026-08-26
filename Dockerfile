# Builder stage
FROM rust:slim AS builder

# Install build dependencies for crates that may need native libs (openssl, etc.)
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates build-essential pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifest files for caching
COPY Cargo.toml Cargo.lock ./
COPY crates/azure_devops_client/Cargo.toml crates/azure_devops_client/
COPY crates/rusty_azure_devops/Cargo.toml crates/rusty_azure_devops/

# Fetch dependencies (improves caching)
RUN cargo fetch

# Copy the rest of the workspace
COPY . .

# Build the server (binary now auto-starts the MCP server; no feature flag required)
RUN cargo build --release -p rusty_azure_devops
RUN strip target/release/ado-mcp || true

# Final image: distroless base for small runtime surface
FROM gcr.io/distroless/cc-debian13

WORKDIR /home/nonroot
COPY --from=builder /app/target/release/ado-mcp /usr/local/bin/ado-mcp

# Run as non-root (distroless includes a nonroot user)
USER nonroot

ENV RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/ado-mcp"]
CMD []
