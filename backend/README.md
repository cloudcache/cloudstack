# QuickStack Backend

Rust/axum PaaS backend. Runs as a systemd service on the K3s master node.

## Prerequisites

- Rust 1.75+
- MySQL 8.0+ (for development)
- `sqlx-cli`: `cargo install sqlx-cli --no-default-features --features mysql`

## Development Setup

```bash
# 1. Copy and fill in env vars
cp .env.example .env

# 2. Create the database
mysql -u root -e "CREATE DATABASE quickstack; CREATE USER 'quickstack'@'%' IDENTIFIED BY 'password'; GRANT ALL ON quickstack.* TO 'quickstack'@'%';"

# 3. Run migrations (creates tables)
export DATABASE_URL=mysql://quickstack:password@localhost:3306/quickstack
sqlx database create
sqlx migrate run --source src/db/migrations

# 4. Generate sqlx offline query cache (needed for builds without a live DB)
cargo sqlx prepare

# 5. Build
cargo build --release
```

## Build without live DB

> **Note:** `cargo check` without `DATABASE_URL` will emit `E0282 type annotations needed`
> for all `sqlx::query!` call sites. This is expected — sqlx can't infer MySQL column types
> at compile time without a database connection. The code is correct; run `cargo sqlx prepare`
> once against a live DB to cache query metadata, then future builds work offline.

After running `cargo sqlx prepare` once, set `SQLX_OFFLINE=true` to compile
against the cached query metadata:

```bash
SQLX_OFFLINE=true cargo build --release
```

## Production Deployment

The binary is installed as a systemd service:

```
/opt/quickstack/quickstack --config /opt/quickstack/config.toml
```

See `docs/design/00-architecture.md` for the full systemd unit file.

## Directory Structure

```
src/
├── main.rs          — entry point: config, DB, AppState, router
├── config.rs        — TOML config types
├── state.rs         — shared AppState (DB pool, crypto, pingora client)
├── error.rs         — AppError → HTTP response
├── crypto.rs        — AES-256-GCM encrypt/decrypt, SHA-256
├── auth/
│   ├── ldap.rs      — LLDAP bind authentication
│   ├── jwt.rs       — HS256 JWT issue/verify
│   └── middleware.rs — axum auth middleware, AuthUser extractor
├── api/
│   ├── auth.rs      — login, logout, me, TOTP
│   ├── users.rs     — admin user management
│   ├── projects.rs  — projects + members (3-level roles)
│   ├── apps.rs      — app CRUD, deploy, pause/resume, env, ports, logs, terminal
│   ├── domains.rs   — domain + pingora proxy host management
│   ├── databases.rs — DB instance provisioning + cluster admin
│   ├── nodes.rs     — node add/remove (triggers SSH provisioning)
│   └── platform.rs  — platform_config read/write
├── k8s/
│   ├── namespace.rs — K8s namespace CRUD
│   ├── deployment.rs — Deployment + NodePort Service, logs, terminal exec
│   ├── pod_spec.rs  — PodSpec builder (LDAP mounts, GPU, anti-affinity)
│   ├── node.rs      — K8s node label/drain operations
│   └── database.rs  — MySQL/PG provisioning + K8s Secret creation
├── proxy/
│   └── pingora.rs   — pingora-proxy-manager REST client
├── ssh/
│   └── mod.rs       — russh node installer + RSA keypair generation
└── db/
    ├── mod.rs       — pool setup + sqlx migrate
    └── migrations/
        └── 001_init_schema.sql
```
