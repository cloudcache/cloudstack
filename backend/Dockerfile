FROM rust:1.87-bookworm AS builder
WORKDIR /app

# Install system deps needed by some crates (openssl, etc.)
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cache dependencies — copy manifests first
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs
RUN cargo build --release && rm -rf src

# Build the real binary
COPY . .
# Touch main.rs so cargo rebuilds it (not the dummy)
RUN touch src/main.rs
ENV SQLX_OFFLINE=true
RUN cargo build --release

# ── Runtime ──────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/quickstack /app/quickstack
COPY --from=builder /app/config /app/config
COPY --from=builder /app/.sqlx /app/.sqlx

ENV RUST_LOG=info,quickstack=debug
EXPOSE 3001

CMD ["/app/quickstack"]
